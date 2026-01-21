use crate::stylesheet::{StylesheetInfo, StylesheetManager};
use tauri::AppHandle;

#[tauri::command]
pub fn list_stylesheets(app: AppHandle) -> Result<Vec<StylesheetInfo>, String> {
    let manager = StylesheetManager::new(&app)?;
    manager.list_stylesheets()
}

#[tauri::command]
pub fn import_stylesheet(app: AppHandle, path: String) -> Result<StylesheetInfo, String> {
    let manager = StylesheetManager::new(&app)?;
    manager.import_stylesheet(&path)
}

#[tauri::command]
pub fn delete_stylesheet(app: AppHandle, id: String) -> Result<(), String> {
    let manager = StylesheetManager::new(&app)?;
    manager.delete_stylesheet(&id)
}
