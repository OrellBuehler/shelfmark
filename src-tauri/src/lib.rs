mod audible_commands;
mod audio_commands;
mod commands;
mod jobs;
mod protocols;
mod state;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            commands::best_metadata,
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
            commands::is_portable,
            commands::restart,
            audio_commands::audio_status,
            audio_commands::audio_add_root,
            audio_commands::audio_update_root,
            audio_commands::audio_remove_root,
            audio_commands::audio_forget_missing,
            audio_commands::audio_scan,
            audio_commands::job_cancel,
            audio_commands::audiobook,
            audio_commands::audio_link,
            audio_commands::audio_unlink,
            audio_commands::audio_reset_link,
            audio_commands::audio_show_in_folder,
            audio_commands::audio_open,
            audio_commands::audio_transfer_plan,
            audio_commands::audio_transfer,
            audible_commands::audible_status,
            audible_commands::audible_connect,
            audible_commands::audible_disconnect,
            audible_commands::audible_sync,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
