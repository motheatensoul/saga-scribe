use crate::importer::tei;
use std::fs;
use std::path::Path;

#[tauri::command]
pub fn import_file(path: String) -> Result<String, String> {
    let path_obj = Path::new(&path);
    let extension = path_obj.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
        
    let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    
    match extension.as_str() {
        "xml" | "tei" => {
            tei::parse(&content)
        },
        _ => Ok(content) // Default to treating as plain text
    }
}
