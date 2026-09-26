mod anomp;
mod audio;
mod library;

#[tauri::command]
fn core_version() -> String {
    anomp::version()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Err(error) = audio::init(app.handle()) {
                eprintln!("[audio] {error}");
            }
            if let Err(error) = library::commands::init(app.handle()) {
                eprintln!("[library] {error}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            core_version,
            audio::audio_device_name,
            audio::play_test_tone,
            audio::stop_test_tone,
            audio::player_load,
            audio::player_set_next,
            audio::player_play,
            audio::player_pause,
            audio::player_stop,
            audio::player_seek,
            audio::player_set_volume,
            audio::player_status,
            library::commands::library_folders,
            library::commands::library_add_folder,
            library::commands::library_remove_folder,
            library::commands::library_scan,
            library::commands::library_browse,
            library::commands::library_sort_settings,
            library::commands::library_save_sort_rule,
            library::commands::library_remove_sort_rule,
            library::commands::library_set_ignored_articles,
            library::commands::library_reset_sort_settings
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                audio::shutdown();
            }
        });
}
