//! # PDF Export
//!
//! Writes a PDF straight to a file the user chose, instead of going through the
//! system print dialog. The document is laid out in a hidden window, so even a
//! very large manuscript never blocks the main window, and is printed there
//! with the platform webview's own PDF support:
//!
//! - **Linux**: a WebKitGTK print operation to GTK's "Print to File" printer
//! - **Windows**: WebView2 `PrintToPdf`
//! - **macOS**: not supported yet; the frontend falls back to the print dialog,
//!   which offers "Save as PDF" itself
//!
//! ## Flow
//!
//! ```text
//! export_pdf ──► opens hidden "pdf-export" window at /print
//!                  │ take_print_document   (page fetches the HTML)
//!                  │ print_document_ready  (page has laid it out)
//!                  ▼
//!                platform print to PDF ──► completion resolves export_pdf
//! ```
//!
//! `cancel_pdf_export` resolves the export early; the hidden window is always
//! destroyed when `export_pdf` returns.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tokio::sync::oneshot;

/// Label of the hidden window the document is printed from.
pub const PRINT_WINDOW: &str = "pdf-export";

/// Error returned by [`export_pdf`] when the user cancelled.
pub const CANCELLED: &str = "PDF export cancelled";

type Outcome = Result<(), String>;

/// Resolves an export exactly once: the first of print completion, failure or
/// cancellation wins.
#[derive(Clone)]
struct Completion(Arc<Mutex<Option<oneshot::Sender<Outcome>>>>);

impl Completion {
    fn complete(&self, outcome: Outcome) {
        if let Ok(mut sender) = self.0.lock() {
            if let Some(sender) = sender.take() {
                let _ = sender.send(outcome);
            }
        }
    }
}

struct PdfJob {
    /// The HTML to print, until the print window has taken it.
    document: Option<String>,
    path: PathBuf,
    completion: Completion,
}

/// The PDF export in progress, if any. Only one runs at a time.
#[derive(Default)]
pub struct PdfExportState(Mutex<Option<PdfJob>>);

/// Whether [`export_pdf`] works on this platform.
#[tauri::command]
pub fn pdf_export_supported() -> bool {
    cfg!(any(target_os = "linux", windows))
}

/// Renders `html` to a PDF at `path` and resolves once the file is written.
#[tauri::command(async)]
pub async fn export_pdf(
    app: AppHandle,
    state: State<'_, PdfExportState>,
    html: String,
    path: String,
) -> Result<(), String> {
    if !pdf_export_supported() {
        return Err("Direct PDF export is not supported on this platform".to_string());
    }

    let (sender, receiver) = oneshot::channel();
    {
        let mut job = state.0.lock().map_err(|_| "PDF export state is poisoned")?;
        if job.is_some() {
            return Err("A PDF export is already running".to_string());
        }
        *job = Some(PdfJob {
            document: Some(html),
            path: PathBuf::from(path),
            completion: Completion(Arc::new(Mutex::new(Some(sender)))),
        });
    }

    let window = WebviewWindowBuilder::new(&app, PRINT_WINDOW, WebviewUrl::App("print".into()))
        .title("PDF export")
        .visible(false)
        .build();
    let outcome = match window {
        Ok(_) => receiver
            .await
            .unwrap_or_else(|_| Err("PDF export was interrupted".to_string())),
        Err(e) => Err(format!("Failed to open the PDF export window: {}", e)),
    };

    if let Some(window) = app.get_webview_window(PRINT_WINDOW) {
        let _ = window.destroy();
    }
    if let Ok(mut job) = state.0.lock() {
        *job = None;
    }
    outcome
}

/// Called by the print window to fetch the document it should display.
#[tauri::command]
pub fn take_print_document(
    window: WebviewWindow,
    state: State<'_, PdfExportState>,
) -> Result<String, String> {
    ensure_print_window(&window)?;
    let mut job = state.0.lock().map_err(|_| "PDF export state is poisoned")?;
    job.as_mut()
        .and_then(|job| job.document.take())
        .ok_or_else(|| "No PDF export is waiting for its document".to_string())
}

/// Called by the print window once the document is laid out; starts printing.
#[tauri::command]
pub fn print_document_ready(
    window: WebviewWindow,
    state: State<'_, PdfExportState>,
) -> Result<(), String> {
    ensure_print_window(&window)?;
    let (path, completion) = {
        let job = state.0.lock().map_err(|_| "PDF export state is poisoned")?;
        let job = job.as_ref().ok_or("No PDF export is running")?;
        (job.path.clone(), job.completion.clone())
    };
    if let Err(e) = print_to_pdf(&window, &path, completion.clone()) {
        completion.complete(Err(e));
    }
    Ok(())
}

/// Called by the print window if it could not display the document.
#[tauri::command]
pub fn print_document_failed(
    window: WebviewWindow,
    state: State<'_, PdfExportState>,
    message: String,
) -> Result<(), String> {
    ensure_print_window(&window)?;
    finish(&state, Err(format!("Preparing the PDF failed: {}", message)));
    Ok(())
}

/// Stops the running PDF export, if any.
#[tauri::command]
pub fn cancel_pdf_export(state: State<'_, PdfExportState>) {
    finish(&state, Err(CANCELLED.to_string()));
}

fn finish(state: &PdfExportState, outcome: Outcome) {
    let completion = state
        .0
        .lock()
        .ok()
        .and_then(|job| job.as_ref().map(|job| job.completion.clone()));
    if let Some(completion) = completion {
        completion.complete(outcome);
    }
}

fn ensure_print_window(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == PRINT_WINDOW {
        Ok(())
    } else {
        Err("Only the PDF export window may call this command".to_string())
    }
}

#[cfg(target_os = "linux")]
fn print_to_pdf(window: &WebviewWindow, path: &Path, completion: Completion) -> Result<(), String> {
    let uri = glib::filename_to_uri(path, None)
        .map_err(|e| format!("Invalid PDF path: {}", e))?
        .to_string();

    window
        .with_webview(move |webview| {
            use webkit2gtk::{PrintOperation, PrintOperationExt};

            let operation = PrintOperation::new(&webview.inner());

            let settings = gtk::PrintSettings::new();
            // GTK registers its file printer under a translated name, so look
            // it up through GTK's own translations rather than hard-coding it.
            settings.set_printer(&glib::dgettext(Some("gtk30"), "Print to File"));
            settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT, Some("pdf"));
            settings.set(gtk::PRINT_SETTINGS_OUTPUT_URI, Some(&uri));
            operation.set_print_settings(&settings);

            let page_setup = gtk::PageSetup::new();
            page_setup.set_paper_size(&gtk::PaperSize::new(Some(gtk::PAPER_NAME_A4)));
            operation.set_page_setup(&page_setup);

            // "failed" is always followed by "finished"; the first outcome wins
            let on_failed = completion.clone();
            operation.connect_failed(move |_, error| {
                on_failed.complete(Err(format!("Printing to PDF failed: {}", error)));
            });
            operation.connect_finished(move |_| completion.complete(Ok(())));

            // Prints without showing a dialog
            operation.print();
        })
        .map_err(|e| format!("Could not access the PDF export window: {}", e))
}

#[cfg(windows)]
fn print_to_pdf(window: &WebviewWindow, path: &Path, completion: Completion) -> Result<(), String> {
    let path = path.to_path_buf();
    window
        .with_webview(move |webview| {
            let started = unsafe {
                windows_print::start(
                    &webview.controller(),
                    &webview.environment(),
                    &path,
                    completion.clone(),
                )
            };
            if let Err(e) = started {
                completion.complete(Err(format!("Printing to PDF failed: {}", e.message())));
            }
        })
        .map_err(|e| format!("Could not access the PDF export window: {}", e))
}

#[cfg(windows)]
mod windows_print {
    use super::Completion;
    use std::path::Path;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Controller, ICoreWebView2Environment, ICoreWebView2Environment6,
        ICoreWebView2_7,
    };
    use webview2_com::PrintToPdfCompletedHandler;
    use windows_core::Interface;
    use windows_strings::HSTRING;

    /// Starts WebView2's PDF printing; `completion` is resolved from its callback.
    ///
    /// # Safety
    ///
    /// Must be called on the thread that owns the WebView2 controller.
    pub unsafe fn start(
        controller: &ICoreWebView2Controller,
        environment: &ICoreWebView2Environment,
        path: &Path,
        completion: Completion,
    ) -> windows_core::Result<()> {
        let webview: ICoreWebView2_7 = controller.CoreWebView2()?.cast()?;
        let settings = environment
            .cast::<ICoreWebView2Environment6>()?
            .CreatePrintSettings()?;
        // A4, in inches
        settings.SetPageWidth(8.27)?;
        settings.SetPageHeight(11.69)?;
        settings.SetShouldPrintBackgrounds(true)?;

        let handler = PrintToPdfCompletedHandler::create(Box::new(move |result, written| {
            completion.complete(match result {
                Ok(()) if written => Ok(()),
                Ok(()) => Err("WebView2 could not write the PDF file".to_string()),
                Err(e) => Err(format!("Printing to PDF failed: {}", e.message())),
            });
            Ok(())
        }));
        webview.PrintToPdf(&HSTRING::from(path), &settings, &handler)
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
fn print_to_pdf(_window: &WebviewWindow, _path: &Path, _completion: Completion) -> Result<(), String> {
    Err("Direct PDF export is not supported on this platform".to_string())
}
