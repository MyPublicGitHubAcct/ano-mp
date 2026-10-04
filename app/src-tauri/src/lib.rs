mod anomp;
mod audio;
#[cfg(test)]
mod bindings;
mod coded;
mod collection;
#[cfg(debug_assertions)]
mod dev;
mod diagnostics;
mod effects;
mod features;
mod history;
mod library;
mod logging;
mod media;
mod metadata;
mod queue;
mod recording;
mod remote;
#[cfg(any(test, feature = "self-test"))]
mod self_test;
mod settings;
mod shell;
mod theme;
mod updates;
mod visualizer;

use tauri::webview::PageLoadEvent;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // The bundle's self-test (PLAN.md H14) runs instead of the app.
    #[cfg(feature = "self-test")]
    if let Some(status) = self_test::run_if_asked() {
        std::process::exit(status);
    }
    // The launch time that the first paint is measured from (PLAN.md H18).
    diagnostics::mark_start();
    // First, so a panic anywhere after is written down (PLAN.md H9).
    logging::install_panic_hook();
    tauri::Builder::default()
        // First, so the other plugins' setup can log.
        .plugin(logging::plugin())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(visualizer::VisualizerState::default())
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Started {
                visualizer::page_loading(webview.app_handle());
            }
        })
        .setup(|app| {
            anomp::forward_core_log();
            log::info!(
                "ano-mp {} (core {}) starting",
                env!("CARGO_PKG_VERSION"),
                anomp::version()
            );
            // The library first: the settings live in it, and name the
            // output device the engine opens.
            if let Err(error) = library::commands::init(app.handle()) {
                log::error!("{error}");
            }
            if let Err(error) = settings::init(app.handle()) {
                log::error!("{error}");
            }
            if let Err(error) = audio::init(app.handle()) {
                log::error!("{error}");
            }
            if let Err(error) = queue::init(app.handle()) {
                log::error!("{error}");
            }
            if let Err(error) = media::init(app.handle()) {
                log::error!("{error}");
            }
            if let Err(error) = metadata::worker::init(app.handle()) {
                log::error!("{error}");
            }
            if let Err(error) = library::analysis::init(app.handle()) {
                log::error!("{error}");
            }
            if let Err(error) = history::init(app.handle()) {
                log::error!("{error}");
            }
            remote::init(app.handle());
            if let Err(error) = updates::init(app.handle()) {
                log::error!("{error}");
            }
            // After the queue: the menus and the Dock follow it.
            if let Err(error) = shell::init(app.handle()) {
                log::error!("{error}");
            }
            // Last: a rescan at launch runs in the background (F9), and the
            // database is checked (H10).
            library::watch::init(app.handle());
            library::commands::upkeep(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            // On macOS, closing the main window hides it and the music plays
            // on; the Dock icon or the menus bring it back.
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (window, event);
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
            // The /dev page's (PLAN.md H2), in debug builds only.
            #[cfg(debug_assertions)]
            dev::core_version,
            #[cfg(debug_assertions)]
            dev::audio_device_name,
            #[cfg(debug_assertions)]
            dev::play_test_tone,
            #[cfg(debug_assertions)]
            dev::stop_test_tone,
            #[cfg(debug_assertions)]
            dev::player_load,
            #[cfg(debug_assertions)]
            dev::player_set_next,
            #[cfg(debug_assertions)]
            dev::player_play,
            #[cfg(debug_assertions)]
            dev::player_pause,
            #[cfg(debug_assertions)]
            dev::player_stop,
            #[cfg(debug_assertions)]
            dev::player_seek,
            audio::audio_output_status,
            settings::settings_get,
            settings::settings_save,
            theme::theme_export,
            theme::theme_import,
            audio::player_set_volume,
            audio::player_status,
            diagnostics::diagnostics_text,
            diagnostics::diagnostics_show_logs,
            diagnostics::diagnostics_notices,
            diagnostics::diagnostics_discogs_notice,
            diagnostics::diagnostics_first_paint,
            library::commands::library_db_check,
            library::commands::library_db_restore,
            library::commands::library_db_rebuild,
            library::commands::library_folders,
            library::commands::library_add_folder,
            library::commands::library_locate_folder,
            library::commands::library_remove_folder,
            library::commands::library_remove_missing,
            library::commands::library_scan,
            library::commands::library_browse,
            library::commands::library_node_track_ids,
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
            queue::queue_move_items,
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
            queue::queue_set_stop_after,
            queue::queue_set_sleep,
            queue::queue_open_files,
            queue::player_set_loop,
            audio::player_set_tempo,
            audio::player_practice,
            audio::player_signal_path,
            effects::effects_catalog,
            effects::effects_preview,
            effects::effects_freeze,
            effects::effects_status,
            recording::recording_status,
            recording::recording_set_folder,
            recording::recording_start,
            recording::recording_stop,
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
            features::library_similar_tracks,
            features::library_similar_albums,
            features::library_similar_artists,
            features::library_artist_page_similar,
            features::library_for_you,
            features::outside_for_you,
            features::outside_like_artist,
            features::outside_links,
            features::outside_dismiss,
            features::outside_status,
            features::outside_forget_dismissed,
            features::library_health,
            features::library_lyrics,
            features::prefs_get,
            features::prefs_set_track,
            features::prefs_set_album,
            collection::playlists_list,
            collection::playlists_page,
            collection::playlists_create,
            collection::playlists_preview,
            collection::playlists_create_from_queue,
            collection::playlists_rename,
            collection::playlists_set_rules,
            collection::playlists_delete,
            collection::playlists_add,
            collection::playlists_remove,
            collection::playlists_move,
            collection::playlists_track_ids,
            collection::playlists_import,
            collection::playlists_export,
            collection::marks_set_favourite,
            collection::marks_favourites_among,
            collection::marks_set_rating,
            collection::marks_favourites,
            collection::library_track_details,
            collection::data_export,
            collection::data_import,
            shell::shell_sort_dropped,
            shell::shell_shortcuts,
            shell::shell_show_main,
            shell::shell_toggle_mini_player,
            remote::remote_status,
            remote::remote_new_code,
            remote::remote_forget,
            updates::updates_status,
            updates::updates_check,
            visualizer::visualizer_subscribe,
            visualizer::visualizer_unsubscribe
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::Exit => {
                log::info!("quitting");
                remote::shutdown(app);
                updates::shutdown(app);
                library::analysis::shutdown(app);
                metadata::worker::shutdown(app);
                recording::shutdown(app);
                queue::shutdown(app);
                library::commands::shutdown(app);
                library::availability::shutdown();
                shell::shutdown();
                media::shutdown();
                audio::shutdown();
            }
            // Files opened from the Finder play (F5).
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            tauri::RunEvent::Opened { urls } => shell::opened(app, urls),
            // The Dock icon clicked with the main window hidden.
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => {
                let _ = shell::show_main(app);
            }
            _ => {}
        });
}
