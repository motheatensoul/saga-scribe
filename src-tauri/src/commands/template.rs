use crate::template::{Template, TemplateManager};
use tauri::AppHandle;

#[tauri::command]
pub fn list_templates(app: AppHandle) -> Result<Vec<Template>, String> {
    let manager = TemplateManager::new(&app)?;
    manager.list_templates()
}

#[tauri::command]
pub fn get_template(app: AppHandle, id: String) -> Result<Template, String> {
    let manager = TemplateManager::new(&app)?;
    manager.get_template(&id)
}

#[tauri::command]
pub fn save_template(app: AppHandle, template: Template) -> Result<(), String> {
    let manager = TemplateManager::new(&app)?;
    manager.save_template(&template)
}
