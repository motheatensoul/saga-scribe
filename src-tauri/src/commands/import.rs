use crate::importer::tei;
use serde::Serialize;
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Emitter};

/// Result payload for import completion event
#[derive(Clone, Serialize)]
pub struct ImportResult {
    pub success: bool,
    pub content: Option<String>,
    pub error: Option<String>,
    pub path: String,
}

/// Start an import operation that runs in the background.
/// The result is emitted as an event to avoid blocking the UI.
#[tauri::command]
pub fn import_file(app: AppHandle, path: String) {
    let app_handle = app.clone();
    let path_clone = path.clone();

    // Spawn the import work on a background thread
    tauri::async_runtime::spawn(async move {
        let result = tauri::async_runtime::spawn_blocking(move || {
            let path_obj = Path::new(&path);
            let extension = path_obj
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;

            match extension.as_str() {
                "xml" | "tei" => tei::parse(&content),
                _ => Ok(content), // Default to treating as plain text
            }
        })
        .await;

        // Emit the result as an event
        let import_result = match result {
            Ok(Ok(content)) => ImportResult {
                success: true,
                content: Some(content),
                error: None,
                path: path_clone,
            },
            Ok(Err(e)) => ImportResult {
                success: false,
                content: None,
                error: Some(e),
                path: path_clone,
            },
            Err(e) => ImportResult {
                success: false,
                content: None,
                error: Some(format!("Task failed: {}", e)),
                path: path_clone,
            },
        };

        let _ = app_handle.emit("import-complete", import_result);
    });
}
