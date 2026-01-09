mod commands;
mod dictionary;
pub mod entities;
mod normalizer;
mod parser;
mod settings;
mod template;

use commands::dictionary::OnpState;
use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(OnpState(Mutex::new(None)))
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Initialize window theme based on settings
            use settings::SettingsManager;
            use tauri::Manager;

            let app_handle = app.handle();
            if let Ok(manager) = SettingsManager::new(&app_handle) {
                let settings = manager.load();
                let theme_str = settings.theme;

                // Determine effective theme
                let effective_theme = if theme_str == "system" {
                    commands::settings::get_system_theme()
                } else {
                    theme_str
                };

                // Set window theme
                if let Some(window) = app_handle.get_webview_window("main") {
                    let window_theme = if effective_theme == "dark" {
                        tauri::Theme::Dark
                    } else {
                        tauri::Theme::Light
                    };
                    let _ = window.set_theme(Some(window_theme));
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::file::open_file,
            commands::file::save_file,
            commands::file::export_tei,
            commands::file::load_text_file,
            commands::file::save_project,
            commands::file::open_project,
            commands::template::list_templates,
            commands::template::get_template,
            commands::template::save_template,
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::settings::get_system_theme,
            commands::settings::set_window_theme,
            commands::parse::compile_dsl,
            commands::entities::load_entities,
            commands::entities::get_entity,
            commands::entities::list_entity_names,
            commands::entities::load_custom_mappings,
            commands::entities::save_entity_mapping,
            commands::entities::remove_entity_mapping,
            commands::entities::clear_custom_mappings,
            commands::dictionary::load_onp_headwords,
            commands::dictionary::lookup_lemma,
            commands::dictionary::search_lemma_prefix,
            commands::dictionary::get_onp_entry,
            commands::dictionary::fetch_onp_full_entry,
            commands::dictionary::load_inflections,
            commands::dictionary::lookup_inflection,
            commands::dictionary::add_inflection,
            commands::dictionary::remove_inflection,
            commands::dictionary::clear_inflections,
            commands::dictionary::export_inflections,
            commands::dictionary::is_onp_loaded,
            commands::dictionary::get_onp_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
