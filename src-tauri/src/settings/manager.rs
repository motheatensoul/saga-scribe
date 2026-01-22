//! # Settings Manager
//!
//! This module provides persistent user settings storage.
//!
//! ## Storage Location
//!
//! Settings are stored in `settings.json` within the app's data directory:
//! - Linux: `~/.local/share/saga-scribe/settings.json`
//! - macOS: `~/Library/Application Support/saga-scribe/settings.json`
//! - Windows: `%APPDATA%\saga-scribe\settings.json`
//!
//! ## Default Values
//!
//! All settings have sensible defaults to ensure the app works out of the box.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// User-configurable application settings.
///
/// Serialized to JSON with camelCase field names for JavaScript interop.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// Editor font size in pixels (default: 14)
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    /// UI theme: "light", "dark", or "system" (default: "system")
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Whether to auto-preview on text changes (default: true)
    #[serde(default = "default_auto_preview")]
    pub auto_preview: bool,
    /// Delay in milliseconds before triggering auto-preview (default: 300)
    #[serde(default = "default_preview_delay")]
    pub preview_delay: u32,
    /// Currently selected TEI template ID (default: None)
    #[serde(default)]
    pub active_template_id: Option<String>,
    /// Currently selected XSLT stylesheet ID (default: "default")
    #[serde(default = "default_active_stylesheet_id")]
    pub active_stylesheet_id: String,
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

fn default_active_stylesheet_id() -> String {
    "default".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            theme: default_theme(),
            auto_preview: default_auto_preview(),
            preview_delay: default_preview_delay(),
            active_template_id: None,
            active_stylesheet_id: default_active_stylesheet_id(),
        }
    }
}

/// Manages persistent user settings storage.
///
/// Settings are stored as JSON in the app's data directory and are loaded
/// with graceful fallback to defaults if the file is missing or corrupted.
pub struct SettingsManager {
    /// Path to settings.json file
    settings_path: PathBuf,
}

impl SettingsManager {
    /// Creates a new settings manager for the given app handle.
    ///
    /// Creates the app data directory if it doesn't exist.
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let app_data = app.path().app_data_dir().map_err(|e| e.to_string())?;

        fs::create_dir_all(&app_data).map_err(|e| e.to_string())?;

        let settings_path = app_data.join("settings.json");

        Ok(Self { settings_path })
    }

    /// Loads settings from disk, returning defaults if file is missing or invalid.
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

    /// Saves settings to disk as pretty-printed JSON.
    pub fn save(&self, settings: &Settings) -> Result<(), String> {
        let content = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
        fs::write(&self.settings_path, content).map_err(|e| e.to_string())
    }
}
