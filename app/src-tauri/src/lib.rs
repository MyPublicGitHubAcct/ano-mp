mod anomp;
mod audio;

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
            audio::player_status
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::Exit = event {
                audio::shutdown();
            }
        });
}
