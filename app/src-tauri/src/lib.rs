mod anomp;
mod audio;
mod features;
mod history;
mod library;
mod media;
mod metadata;
mod queue;
mod remote;
mod settings;
mod visualizer;

use tauri::webview::PageLoadEvent;
use tauri::Manager;

#[tauri::command]
fn core_version() -> String {
    anomp::version()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(visualizer::VisualizerState::default())
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Started {
                visualizer::page_loading(webview.app_handle());
            }
        })
        .setup(|app| {
            // The library first: the settings live in it, and name the
            // output device the engine opens.
            if let Err(error) = library::commands::init(app.handle()) {
                eprintln!("[library] {error}");
            }
            if let Err(error) = settings::init(app.handle()) {
                eprintln!("[settings] {error}");
            }
            if let Err(error) = audio::init(app.handle()) {
                eprintln!("[audio] {error}");
            }
            if let Err(error) = queue::init(app.handle()) {
                eprintln!("[queue] {error}");
            }
            if let Err(error) = media::init(app.handle()) {
                eprintln!("[media] {error}");
            }
            if let Err(error) = metadata::worker::init(app.handle()) {
                eprintln!("[metadata] {error}");
            }
            if let Err(error) = library::analysis::init(app.handle()) {
                eprintln!("[analysis] {error}");
            }
            if let Err(error) = history::init(app.handle()) {
                eprintln!("[history] {error}");
            }
            remote::init(app.handle());
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol(
            library::art::SCHEME,
            |ctx, request, responder| {
                let app = ctx.app_handle().clone();
                let path = request.uri().path().to_owned();
                let query = request.uri().query().map(str::to_owned);
                tauri::async_runtime::spawn_blocking(move || {
                    let library = app.try_state::<library::commands::LibraryState>();
                    responder.respond(library::art::respond(
                        library.as_deref(),
                        &path,
                        query.as_deref(),
                    ));
                });
            },
        )
        .invoke_handler(tauri::generate_handler![
            core_version,
            audio::audio_device_name,
            audio::audio_output_status,
            settings::settings_get,
            settings::settings_save,
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
            library::commands::library_search,
            library::commands::library_artist,
            library::commands::library_cover_wall,
            library::commands::library_sort_settings,
            library::commands::library_save_sort_rule,
            library::commands::library_remove_sort_rule,
            library::commands::library_set_ignored_articles,
            library::commands::library_reset_sort_settings,
            metadata::commands::metadata_settings,
            metadata::commands::metadata_save_settings,
            metadata::commands::metadata_reset_settings,
            metadata::commands::metadata_set_key,
            metadata::commands::metadata_status,
            metadata::commands::metadata_retry_now,
            metadata::commands::metadata_update_album,
            metadata::commands::metadata_update_artist,
            metadata::commands::metadata_album,
            metadata::commands::metadata_release_candidates,
            metadata::commands::metadata_choose_release,
            metadata::commands::metadata_release_details,
            metadata::commands::metadata_reject_release,
            metadata::commands::metadata_use_automatic_release,
            metadata::commands::metadata_cover_candidates,
            metadata::commands::metadata_fetch_image,
            metadata::commands::metadata_choose_cover,
            metadata::commands::metadata_use_automatic_cover,
            metadata::commands::metadata_artist_candidates,
            metadata::commands::metadata_artist_discography,
            metadata::commands::metadata_choose_artist,
            metadata::commands::metadata_reject_artist,
            metadata::commands::metadata_use_automatic_artist,
            queue::queue_state,
            queue::queue_play,
            queue::queue_play_node,
            queue::queue_add,
            queue::queue_add_node,
            queue::queue_remove,
            queue::queue_move,
            queue::queue_clear,
            queue::queue_jump,
            queue::queue_next,
            queue::queue_previous,
            queue::queue_toggle,
            queue::queue_seek,
            queue::queue_set_shuffle,
            queue::queue_set_repeat,
            queue::queue_start_radio,
            queue::queue_stop_radio,
            queue::player_set_loop,
            audio::player_set_tempo,
            audio::player_practice,
            audio::player_signal_path,
            features::analysis_status,
            features::analysis_waveform,
            features::analysis_track,
            features::history_recent,
            features::history_top,
            features::history_highlights,
            features::history_clear,
            features::history_listenbrainz_status,
            features::history_set_listenbrainz_token,
            features::library_recently_added,
            features::library_on_this_day,
            features::library_more_in_genre,
            features::library_health,
            features::library_lyrics,
            features::prefs_get,
            features::prefs_set_track,
            features::prefs_set_album,
            remote::remote_status,
            remote::remote_new_code,
            remote::remote_forget,
            visualizer::visualizer_subscribe,
            visualizer::visualizer_unsubscribe
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                remote::shutdown(app);
                library::analysis::shutdown(app);
                metadata::worker::shutdown(app);
                queue::shutdown(app);
                media::shutdown();
                audio::shutdown();
            }
        });
}
