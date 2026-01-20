use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const DEFAULT_STYLESHEET_ID: &str = "default";
const DEFAULT_STYLESHEET_NAME: &str = "Default (simple.xsl)";
const DEFAULT_STYLESHEET_PATH: &str = "/xsl/simple.xsl";
const MANIFEST_FILE: &str = "manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StylesheetMetadata {
    pub id: String,
    pub name: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StylesheetInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub built_in: bool,
}

pub struct StylesheetManager {
    stylesheets_dir: PathBuf,
    manifest_path: PathBuf,
}

impl StylesheetManager {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;
        let stylesheets_dir = app_data.join("stylesheets");
        fs::create_dir_all(&stylesheets_dir).map_err(|e| e.to_string())?;
        let manifest_path = stylesheets_dir.join(MANIFEST_FILE);

        Ok(Self {
            stylesheets_dir,
            manifest_path,
        })
    }

    pub fn list_stylesheets(&self) -> Result<Vec<StylesheetInfo>, String> {
        let mut entries = vec![StylesheetInfo {
            id: DEFAULT_STYLESHEET_ID.to_string(),
            name: DEFAULT_STYLESHEET_NAME.to_string(),
            path: DEFAULT_STYLESHEET_PATH.to_string(),
            built_in: true,
        }];

        let mut manifest = self.load_manifest();
        manifest.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        for entry in manifest {
            let path = self
                .stylesheets_dir
                .join(&entry.file_name)
                .to_string_lossy()
                .to_string();
            entries.push(StylesheetInfo {
                id: entry.id,
                name: entry.name,
                path,
                built_in: false,
            });
        }

        Ok(entries)
    }

    pub fn import_stylesheet(&self, source_path: &str) -> Result<StylesheetInfo, String> {
        let source = PathBuf::from(source_path);
        if !source.exists() {
            return Err("Stylesheet path does not exist".to_string());
        }

        let file_stem = source
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("stylesheet");
        let display_name = if file_stem.trim().is_empty() {
            "Stylesheet"
        } else {
            file_stem
        };
        let extension = source
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("xsl");

        let mut manifest = self.load_manifest();
        let base_id = sanitize_id(display_name);
        let id = unique_id(&base_id, &manifest);
        let file_name = format!("{}.{}", id, extension);
        let target_path = self.stylesheets_dir.join(&file_name);

        fs::copy(&source, &target_path).map_err(|e| e.to_string())?;

        let metadata = StylesheetMetadata {
            id: id.clone(),
            name: display_name.to_string(),
            file_name: file_name.clone(),
        };
        manifest.push(metadata.clone());
        self.save_manifest(&manifest)?;

        Ok(StylesheetInfo {
            id,
            name: metadata.name,
            path: target_path.to_string_lossy().to_string(),
            built_in: false,
        })
    }

    pub fn delete_stylesheet(&self, id: &str) -> Result<(), String> {
        if id == DEFAULT_STYLESHEET_ID {
            return Err("Cannot delete the default stylesheet".to_string());
        }

        let mut manifest = self.load_manifest();
        let index = manifest
            .iter()
            .position(|entry| entry.id == id)
            .ok_or_else(|| "Stylesheet not found".to_string())?;
        let entry = manifest.remove(index);
        let target_path = self.stylesheets_dir.join(entry.file_name);

        if target_path.exists() {
            fs::remove_file(&target_path).map_err(|e| e.to_string())?;
        }

        self.save_manifest(&manifest)
    }

    fn load_manifest(&self) -> Vec<StylesheetMetadata> {
        if !self.manifest_path.exists() {
            return Vec::new();
        }

        match fs::read_to_string(&self.manifest_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    fn save_manifest(&self, manifest: &[StylesheetMetadata]) -> Result<(), String> {
        let content = serde_json::to_string_pretty(manifest).map_err(|e| e.to_string())?;
        fs::write(&self.manifest_path, content).map_err(|e| e.to_string())
    }
}

fn sanitize_id(name: &str) -> String {
    let mut output = String::new();
    let mut last_dash = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            output.push('-');
            last_dash = true;
        }
    }

    let trimmed = output.trim_matches('-');
    if trimmed.is_empty() {
        "stylesheet".to_string()
    } else {
        trimmed.to_string()
    }
}

fn unique_id(base_id: &str, manifest: &[StylesheetMetadata]) -> String {
    let mut candidate = if base_id.is_empty() {
        "stylesheet".to_string()
    } else {
        base_id.to_string()
    };
    let mut counter = 2;

    while candidate == DEFAULT_STYLESHEET_ID
        || manifest.iter().any(|entry| entry.id == candidate)
    {
        candidate = format!("{}-{}", base_id, counter);
        counter += 1;
    }

    candidate
}
