use crate::entities::{CustomMappingsManager, Entity, EntityRegistry};
use log::{debug, error, info};
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;

/// Load entities from a JSON file and return them as a map
#[tauri::command]
pub fn load_entities(path: String) -> Result<HashMap<String, Entity>, String> {
    info!("Loading entities from: {}", path);

    let content = fs::read_to_string(&path).map_err(|e| {
        error!("Failed to read entity file {}: {}", path, e);
        format!("Failed to read file: {}", e)
    })?;

    debug!("Read {} bytes from entity file", content.len());

    let mut registry = EntityRegistry::new();
    registry.load_from_str(&content).map_err(|e| {
        error!("Failed to parse entity file: {}", e);
        e
    })?;

    let count = registry.to_map().len();
    info!("Successfully loaded {} entities", count);

    Ok(registry.to_map().clone())
}

/// Get a single entity by name from a loaded entity file
#[tauri::command]
pub fn get_entity(path: String, name: String) -> Result<Option<Entity>, String> {
    let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;

    let mut registry = EntityRegistry::new();
    registry.load_from_str(&content)?;

    Ok(registry.get(&name).cloned())
}

/// List all entity names from a loaded entity file
#[tauri::command]
pub fn list_entity_names(path: String) -> Result<Vec<String>, String> {
    let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;

    let mut registry = EntityRegistry::new();
    registry.load_from_str(&content)?;

    Ok(registry.names().into_iter().cloned().collect())
}

/// Load custom entity mappings from the app data directory
#[tauri::command]
pub fn load_custom_mappings(app: AppHandle) -> Result<HashMap<String, String>, String> {
    let manager = CustomMappingsManager::new(&app)?;
    Ok(manager.load())
}

/// Save a custom entity mapping
#[tauri::command]
pub fn save_entity_mapping(
    app: AppHandle,
    entity: String,
    translation: String,
) -> Result<(), String> {
    info!("Saving custom mapping: {} -> {}", entity, translation);
    let manager = CustomMappingsManager::new(&app)?;
    manager.save(&entity, &translation)
}

/// Remove a custom entity mapping
#[tauri::command]
pub fn remove_entity_mapping(app: AppHandle, entity: String) -> Result<(), String> {
    info!("Removing custom mapping: {}", entity);
    let manager = CustomMappingsManager::new(&app)?;
    manager.remove(&entity)
}

/// Clear all custom entity mappings
#[tauri::command]
pub fn clear_custom_mappings(app: AppHandle) -> Result<(), String> {
    info!("Clearing all custom mappings");
    let manager = CustomMappingsManager::new(&app)?;
    manager.clear()
}
