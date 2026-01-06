use crate::settings::{Settings, SettingsManager};
use tauri::AppHandle;

#[tauri::command]
pub fn load_settings(app: AppHandle) -> Result<Settings, String> {
    let manager = SettingsManager::new(&app)?;
    Ok(manager.load())
}

#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let manager = SettingsManager::new(&app)?;
    manager.save(&settings)
}
