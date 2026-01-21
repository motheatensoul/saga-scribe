use crate::importer::tei::{self, ImportResult};
use std::fs;
use std::path::Path;

/// Import a file and convert it to DSL format, also extracting metadata if available.
///
/// This command is async, which means Tauri executes it on a separate async task
/// (not the main thread), preventing UI blocking. The actual file I/O and parsing
/// runs on a blocking thread pool via spawn_blocking.
#[tauri::command(async)]
pub async fn import_file(path: String) -> Result<ImportResult, String> {
    // spawn_blocking moves the CPU-bound work to a thread pool,
    // while the async command itself runs off the main thread
    tauri::async_runtime::spawn_blocking(move || {
        let path_obj = Path::new(&path);
        let extension = path_obj
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

        match extension.as_str() {
            "xml" | "tei" => tei::parse(&content),
            // Plain text files - return as DSL with no metadata or segments
            _ => Ok(ImportResult {
                dsl: content,
                metadata: None,
                imported_document: None,
                original_body_xml: None,
                original_preamble: None,
                original_postamble: None,
                is_imported_mode: false,
            }),
        }
    })
    .await
    .map_err(|e| format!("Import task failed: {}", e))?
}
