mod commands;
mod protocols;
mod state;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol("cover", protocols::cover)
        .register_asynchronous_uri_scheme_protocol("bookfile", protocols::bookfile)
        .setup(|app| {
            let state = state::AppState::load(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::current_library,
            commands::recent_libraries,
            commands::open_library,
            commands::create_library,
            commands::list_books,
            commands::get_book,
            commands::get_categories,
            commands::completions,
            commands::saved_searches,
            commands::set_saved_searches,
            commands::update_book,
            commands::bulk_update,
            commands::set_cover_from_file,
            commands::set_cover_from_url,
            commands::remove_cover,
            commands::embed_metadata,
            commands::add_books,
            commands::add_format,
            commands::remove_format,
            commands::delete_books,
            commands::show_in_folder,
            commands::open_externally,
            commands::identify,
            commands::detect_devices,
            commands::add_folder_device,
            commands::device_books,
            commands::send_to_device,
            commands::import_from_device,
            commands::delete_device_book,
            commands::reading_progress,
            commands::reading_state,
            commands::set_reading_state,
            commands::annotations,
            commands::save_annotation,
            commands::delete_annotation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
