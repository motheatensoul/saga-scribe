use super::registry::Entity;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomEntitiesFile {
    pub version: String,
    pub entities: HashMap<String, Entity>,
}

impl Default for CustomEntitiesFile {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            entities: HashMap::new(),
        }
    }
}

pub struct CustomEntitiesManager {
    path: PathBuf,
}

impl CustomEntitiesManager {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
        fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;
        let path = app_data.join("custom-entities.json");
        Ok(Self { path })
    }

    /// Load all custom entities
    pub fn load(&self) -> HashMap<String, Entity> {
        if self.path.exists() {
            if let Ok(content) = fs::read_to_string(&self.path) {
                if let Ok(file) = serde_json::from_str::<CustomEntitiesFile>(&content) {
                    return file.entities;
                }
            }
        }
        HashMap::new()
    }

    /// Save or update a custom entity
    pub fn save(&self, name: &str, entity: Entity) -> Result<(), String> {
        if !Self::is_valid_name(name) {
            return Err(
                "Entity name must contain only alphanumeric characters and underscores".to_string(),
            );
        }

        let mut file = self.load_file();
        file.entities.insert(name.to_string(), entity);
        self.write_file(&file)
    }

    /// Remove a custom entity
    pub fn remove(&self, name: &str) -> Result<(), String> {
        let mut file = self.load_file();
        file.entities.remove(name);
        self.write_file(&file)
    }

    /// Clear all custom entities
    pub fn clear(&self) -> Result<(), String> {
        let file = CustomEntitiesFile::default();
        self.write_file(&file)
    }

    /// Validate entity name (alphanumeric + underscore only, must start with letter)
    pub fn is_valid_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let mut chars = name.chars();
        // First character must be a letter
        if let Some(first) = chars.next() {
            if !first.is_ascii_alphabetic() {
                return false;
            }
        }
        // Rest can be alphanumeric or underscore
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    fn load_file(&self) -> CustomEntitiesFile {
        if self.path.exists() {
            if let Ok(content) = fs::read_to_string(&self.path) {
                if let Ok(file) = serde_json::from_str::<CustomEntitiesFile>(&content) {
                    return file;
                }
            }
        }
        CustomEntitiesFile::default()
    }

    fn write_file(&self, file: &CustomEntitiesFile) -> Result<(), String> {
        let content = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
        fs::write(&self.path, content).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_name() {
        // Valid names
        assert!(CustomEntitiesManager::is_valid_name("foo"));
        assert!(CustomEntitiesManager::is_valid_name("foo_bar"));
        assert!(CustomEntitiesManager::is_valid_name("foo123"));
        assert!(CustomEntitiesManager::is_valid_name("a"));
        assert!(CustomEntitiesManager::is_valid_name("myEntity"));

        // Invalid names
        assert!(!CustomEntitiesManager::is_valid_name("")); // Empty
        assert!(!CustomEntitiesManager::is_valid_name("123foo")); // Starts with number
        assert!(!CustomEntitiesManager::is_valid_name("_foo")); // Starts with underscore
        assert!(!CustomEntitiesManager::is_valid_name("foo-bar")); // Contains hyphen
        assert!(!CustomEntitiesManager::is_valid_name("foo bar")); // Contains space
        assert!(!CustomEntitiesManager::is_valid_name("foo:bar")); // Contains colon
    }
}
