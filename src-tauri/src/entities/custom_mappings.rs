use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomMappingsFile {
    pub version: String,
    pub mappings: HashMap<String, String>,
}

impl Default for CustomMappingsFile {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            mappings: HashMap::new(),
        }
    }
}

pub struct CustomMappingsManager {
    path: PathBuf,
}

impl CustomMappingsManager {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
        fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;
        let path = app_data.join("custom-entity-mappings.json");
        Ok(Self { path })
    }

    pub fn load(&self) -> HashMap<String, String> {
        if self.path.exists() {
            if let Ok(content) = fs::read_to_string(&self.path) {
                if let Ok(file) = serde_json::from_str::<CustomMappingsFile>(&content) {
                    return file.mappings;
                }
            }
        }
        HashMap::new()
    }

    pub fn save(&self, entity: &str, translation: &str) -> Result<(), String> {
        let mut file = self.load_file();
        file.mappings.insert(entity.to_string(), translation.to_string());
        self.write_file(&file)
    }

    pub fn remove(&self, entity: &str) -> Result<(), String> {
        let mut file = self.load_file();
        file.mappings.remove(entity);
        self.write_file(&file)
    }

    pub fn clear(&self) -> Result<(), String> {
        let file = CustomMappingsFile::default();
        self.write_file(&file)
    }

    fn load_file(&self) -> CustomMappingsFile {
        if self.path.exists() {
            if let Ok(content) = fs::read_to_string(&self.path) {
                if let Ok(file) = serde_json::from_str::<CustomMappingsFile>(&content) {
                    return file;
                }
            }
        }
        CustomMappingsFile::default()
    }

    fn write_file(&self, file: &CustomMappingsFile) -> Result<(), String> {
        let content = serde_json::to_string_pretty(file).map_err(|e| e.to_string())?;
        fs::write(&self.path, content).map_err(|e| e.to_string())
    }
}
