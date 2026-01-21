use crate::entities::{CustomEntitiesManager, CustomMappingsManager, Entity, EntityRegistry};
use log::{debug, error, info};
use std::collections::HashMap;
use std::fs;
use tauri::AppHandle;

/// Load entities from a JSON file and return them as a map
#[tauri::command(async)]
pub async fn load_entities(path: String) -> Result<HashMap<String, Entity>, String> {
    tauri::async_runtime::spawn_blocking(move || {
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
    })
    .await
    .map_err(|e| format!("Entity load task failed: {}", e))?
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
#[tauri::command(async)]
pub async fn load_custom_mappings(app: AppHandle) -> Result<HashMap<String, String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = CustomMappingsManager::new(&app)?;
        Ok(manager.load())
    })
    .await
    .map_err(|e| format!("Custom mappings task failed: {}", e))?
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

// ============================================================================
// Custom Entity Definition Commands (not just mappings - full entity CRUD)
// ============================================================================

/// Load all custom entity definitions from the app data directory
#[tauri::command(async)]
pub async fn load_custom_entities(app: AppHandle) -> Result<HashMap<String, Entity>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = CustomEntitiesManager::new(&app)?;
        Ok(manager.load())
    })
    .await
    .map_err(|e| format!("Custom entities task failed: {}", e))?
}

/// Save a custom entity definition
#[tauri::command(rename_all = "camelCase")]
pub fn save_custom_entity(
    app: AppHandle,
    name: String,
    unicode: String,
    char_value: String,
    description: String,
    category: String,
) -> Result<(), String> {
    info!("Saving custom entity: {}", name);
    let manager = CustomEntitiesManager::new(&app)?;
    let entity = Entity {
        unicode,
        char: char_value,
        description,
        category,
    };
    manager.save(&name, entity)
}

/// Remove a custom entity definition
#[tauri::command]
pub fn remove_custom_entity(app: AppHandle, name: String) -> Result<(), String> {
    info!("Removing custom entity: {}", name);
    let manager = CustomEntitiesManager::new(&app)?;
    manager.remove(&name)
}

/// Clear all custom entity definitions
#[tauri::command]
pub fn clear_custom_entities(app: AppHandle) -> Result<(), String> {
    info!("Clearing all custom entities");
    let manager = CustomEntitiesManager::new(&app)?;
    manager.clear()
}

/// Validate an entity name (alphanumeric + underscore, must start with letter)
#[tauri::command]
pub fn validate_entity_name(name: String) -> bool {
    CustomEntitiesManager::is_valid_name(&name)
}
