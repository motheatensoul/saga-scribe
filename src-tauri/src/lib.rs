mod commands;
pub mod entities;
mod normalizer;
mod parser;
mod settings;
mod template;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::file::open_file,
            commands::file::save_file,
            commands::file::export_tei,
            commands::file::load_text_file,
            commands::template::list_templates,
            commands::template::get_template,
            commands::template::save_template,
            commands::settings::load_settings,
            commands::settings::save_settings,
            commands::parse::compile_dsl,
            commands::entities::load_entities,
            commands::entities::get_entity,
            commands::entities::list_entity_names,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
