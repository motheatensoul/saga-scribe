use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_auto_preview")]
    pub auto_preview: bool,
    #[serde(default = "default_preview_delay")]
    pub preview_delay: u32,
    #[serde(default)]
    pub active_template_id: Option<String>,
}

fn default_font_size() -> u32 {
    14
}

fn default_theme() -> String {
    "system".to_string()
}

fn default_auto_preview() -> bool {
    true
}

fn default_preview_delay() -> u32 {
    300
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            theme: default_theme(),
            auto_preview: default_auto_preview(),
            preview_delay: default_preview_delay(),
            active_template_id: None,
        }
    }
}

pub struct SettingsManager {
    settings_path: PathBuf,
}

impl SettingsManager {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;

        fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;

        let settings_path = app_data.join("settings.json");

        Ok(Self { settings_path })
    }

    pub fn load(&self) -> Settings {
        if self.settings_path.exists() {
            if let Ok(content) = fs::read_to_string(&self.settings_path) {
                if let Ok(settings) = serde_json::from_str::<Settings>(&content) {
                    return settings;
                }
            }
        }
        Settings::default()
    }

    pub fn save(&self, settings: &Settings) -> Result<(), String> {
        let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        fs::write(&self.settings_path, content).map_err(|e| e.to_string())
    }
}
