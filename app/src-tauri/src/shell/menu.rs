//! The menu bar (PLAN.md F6): the app, File, Edit, Controls, View, Window
//! and Help menus, with shortcuts that work wherever the focus is (a
//! page's keydown handler loses them to a focused text field).
//!
//! Transport items (play, next, previous, volume, shuffle, repeat, stop
//! after, the sleep timer) run here, through the queue functions the media
//! keys use. The rest are the page's to do: they reach it as a `menu`
//! event naming the item (`MenuAction`).

use tauri::menu::{
    AboutMetadata, CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItem,
    MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Manager, Runtime, Wry};

use crate::audio;
use crate::queue::model::{QueueState, Repeat, SleepTimer};
use crate::queue::{self, SleepRequest};

/// Frontend event naming a menu item the page handles.
pub const MENU_EVENT: &str = "menu";

/// Volume steps from the menu.
const VOLUME_STEP: f64 = 0.05;

/// Menu items whose text or check follows the queue and player.
pub struct MenuState<R: Runtime> {
    play: MenuItem<R>,
    shuffle: CheckMenuItem<R>,
    repeat: [CheckMenuItem<R>; 3],
    stop_after: CheckMenuItem<R>,
    sleep: Vec<(Option<SleepChoice>, CheckMenuItem<R>)>,
}

/// A sleep timer the menu offers.
#[derive(Debug, Clone, Copy, PartialEq)]
enum SleepChoice {
    Minutes(u32),
    EndOfTrack,
    EndOfAlbum,
}

impl SleepChoice {
    fn request(self) -> SleepRequest {
        match self {
            SleepChoice::Minutes(minutes) => SleepRequest::Minutes { minutes },
            SleepChoice::EndOfTrack => SleepRequest::EndOfTrack,
            SleepChoice::EndOfAlbum => SleepRequest::EndOfAlbum,
        }
    }
}

const SLEEP_CHOICES: [(&str, &str, Option<SleepChoice>); 7] = [
    ("sleep-off", "Off", None),
    ("sleep-15", "In 15 Minutes", Some(SleepChoice::Minutes(15))),
    ("sleep-30", "In 30 Minutes", Some(SleepChoice::Minutes(30))),
    ("sleep-60", "In 1 Hour", Some(SleepChoice::Minutes(60))),
    ("sleep-90", "In 90 Minutes", Some(SleepChoice::Minutes(90))),
    (
        "sleep-track",
        "At the End of This Track",
        Some(SleepChoice::EndOfTrack),
    ),
    (
        "sleep-album",
        "At the End of This Album",
        Some(SleepChoice::EndOfAlbum),
    ),
];

/// Items the page handles, by id, with their shortcuts. The shortcuts
/// sheet lists these too.
const PAGE_ITEMS: [(&str, &str, Option<&str>); 21] = [
    ("settings", "Settings…", Some("CmdOrCtrl+,")),
    (
        "add-folder",
        "Add Folder to Library…",
        Some("CmdOrCtrl+Shift+O"),
    ),
    ("open-files", "Open Files…", Some("CmdOrCtrl+O")),
    ("new-playlist", "New Playlist", Some("CmdOrCtrl+N")),
    (
        "new-smart-playlist",
        "New Smart Playlist…",
        Some("Alt+CmdOrCtrl+N"),
    ),
    ("import-playlist", "Import Playlist…", None),
    ("export-playlist", "Export Playlist…", None),
    ("export-data", "Export Library Data…", None),
    ("import-data", "Import Library Data…", None),
    ("find", "Find", Some("CmdOrCtrl+F")),
    ("get-info", "Get Info", Some("CmdOrCtrl+I")),
    ("go-to-current", "Go to Current Track", Some("CmdOrCtrl+L")),
    ("show-home", "Home", Some("CmdOrCtrl+1")),
    ("show-library", "Library", Some("CmdOrCtrl+2")),
    ("show-favourites", "Favourites", Some("CmdOrCtrl+3")),
    ("show-now-playing", "Now Playing", Some("CmdOrCtrl+4")),
    ("show-queue", "Queue", Some("CmdOrCtrl+5")),
    ("show-visualizer", "Visualizer", Some("CmdOrCtrl+6")),
    (
        "toggle-queue",
        "Show or Hide the Queue Panel",
        Some("Alt+CmdOrCtrl+Q"),
    ),
    ("mini-player", "Mini Player", Some("Alt+CmdOrCtrl+M")),
    ("shortcuts", "Keyboard Shortcuts", Some("CmdOrCtrl+/")),
];

fn page_item<R: Runtime>(app: &AppHandle<R>, id: &str) -> tauri::Result<MenuItem<R>> {
    let (_, text, accelerator) = PAGE_ITEMS
        .iter()
        .find(|(item, ..)| *item == id)
        .expect("a page item");
    let mut builder = MenuItemBuilder::with_id(id, *text);
    if let Some(accelerator) = accelerator {
        builder = builder.accelerator(*accelerator);
    }
    builder.build(app)
}

/// Builds the menu bar and handles its items. Call in `setup`.
pub fn install(app: &AppHandle<Wry>) -> tauri::Result<()> {
    let name = app.package_info().name.clone();
    let item = |id: &str| page_item(app, id);

    let app_menu = SubmenuBuilder::new(app, &name)
        .item(&PredefinedMenuItem::about(
            app,
            None,
            Some(AboutMetadata {
                name: Some(name.clone()),
                version: Some(app.package_info().version.to_string()),
                ..AboutMetadata::default()
            }),
        )?)
        .separator()
        .item(&item("settings")?)
        .separator()
        .item(&PredefinedMenuItem::services(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app, None)?)
        .item(&PredefinedMenuItem::hide_others(app, None)?)
        .item(&PredefinedMenuItem::show_all(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app, None)?)
        .build()?;

    let file = SubmenuBuilder::new(app, "File")
        .item(&item("new-playlist")?)
        .item(&item("new-smart-playlist")?)
        .separator()
        .item(&item("open-files")?)
        .item(&item("add-folder")?)
        .separator()
        .item(&item("import-playlist")?)
        .item(&item("export-playlist")?)
        .separator()
        .item(&item("import-data")?)
        .item(&item("export-data")?)
        .separator()
        .item(&item("get-info")?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app, None)?)
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .separator()
        .item(&item("find")?)
        .build()?;

    let play = MenuItemBuilder::with_id("play", "Play").build(app)?;
    let shuffle = CheckMenuItemBuilder::with_id("shuffle", "Shuffle").build(app)?;
    let repeat = [
        CheckMenuItemBuilder::with_id("repeat-off", "Off").build(app)?,
        CheckMenuItemBuilder::with_id("repeat-all", "All").build(app)?,
        CheckMenuItemBuilder::with_id("repeat-one", "One").build(app)?,
    ];
    let stop_after = CheckMenuItemBuilder::with_id("stop-after", "Stop After This Track")
        .accelerator("Alt+CmdOrCtrl+Period")
        .build(app)?;
    let mut sleep = Vec::new();
    let mut sleep_menu = SubmenuBuilder::new(app, "Sleep Timer");
    for (id, text, choice) in SLEEP_CHOICES {
        let check = CheckMenuItemBuilder::with_id(id, text).build(app)?;
        sleep_menu = sleep_menu.item(&check);
        if choice.is_none() {
            sleep_menu = sleep_menu.separator();
        }
        sleep.push((choice, check));
    }
    let controls = SubmenuBuilder::new(app, "Controls")
        .item(&play)
        .item(
            &MenuItemBuilder::with_id("next", "Next")
                .accelerator("CmdOrCtrl+Right")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("previous", "Previous")
                .accelerator("CmdOrCtrl+Left")
                .build(app)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("volume-up", "Increase Volume")
                .accelerator("CmdOrCtrl+Up")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id("volume-down", "Decrease Volume")
                .accelerator("CmdOrCtrl+Down")
                .build(app)?,
        )
        .separator()
        .item(&shuffle)
        .item(
            &SubmenuBuilder::new(app, "Repeat")
                .item(&repeat[0])
                .item(&repeat[1])
                .item(&repeat[2])
                .build()?,
        )
        .separator()
        .item(&stop_after)
        .item(&sleep_menu.build()?)
        .separator()
        .item(&item("go-to-current")?)
        .build()?;

    let view = SubmenuBuilder::new(app, "View")
        .item(&item("show-home")?)
        .item(&item("show-library")?)
        .item(&item("show-favourites")?)
        .item(&item("show-now-playing")?)
        .item(&item("show-queue")?)
        .item(&item("show-visualizer")?)
        .separator()
        .item(&item("toggle-queue")?)
        .item(&item("mini-player")?)
        .separator()
        .item(&PredefinedMenuItem::fullscreen(app, None)?)
        .build()?;

    let window = SubmenuBuilder::new(app, "Window")
        .item(&PredefinedMenuItem::minimize(app, None)?)
        .item(&PredefinedMenuItem::maximize(app, None)?)
        .build()?;

    // Logs join it with H9 (PLAN.md Phase 7).
    let help = SubmenuBuilder::new(app, "Help")
        .item(&item("shortcuts")?)
        .build()?;

    let menu: Menu<Wry> = MenuBuilder::new(app)
        .items(&[&app_menu, &file, &edit, &controls, &view, &window, &help])
        .build()?;
    app.set_menu(menu)?;
    app.manage(MenuState {
        play,
        shuffle,
        repeat,
        stop_after,
        sleep,
    });
    app.on_menu_event(|app, event| handle(app, event.id().as_ref()));
    Ok(())
}

/// Runs a menu item (from the menu bar, the Dock or the tray menu). Main
/// thread.
pub fn handle<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let result = match id {
        "play" => queue::toggle(app),
        "next" => queue::next(app),
        "previous" => queue::previous(app),
        "volume-up" | "volume-down" => {
            let step = if id == "volume-up" {
                VOLUME_STEP
            } else {
                -VOLUME_STEP
            };
            audio::change_volume(app, step).map(|volume| {
                let _ = app.emit(audio::PLAYER_VOLUME_EVENT, volume);
            })
        }
        "shuffle" => queue::toggle_shuffle(app),
        "repeat-off" => queue::set_repeat(app, Repeat::Off),
        "repeat-all" => queue::set_repeat(app, Repeat::All),
        "repeat-one" => queue::set_repeat(app, Repeat::One),
        "stop-after" => queue::toggle_stop_after_current(app),
        "mini-player" => super::mini::toggle(app),
        "show-main" => super::show_main(app),
        _ => {
            if let Some((_, _, choice)) = SLEEP_CHOICES.iter().find(|(item, ..)| *item == id) {
                queue::queue_set_sleep(app.clone(), choice.map(SleepChoice::request))
            } else if PAGE_ITEMS.iter().any(|(item, ..)| *item == id) {
                // The page's to do, in the main window, which is brought
                // to the front for it.
                super::show_main(app).and_then(|()| {
                    app.emit_to("main", MENU_EVENT, id)
                        .map_err(|e| e.to_string())
                })
            } else {
                Ok(())
            }
        }
    };
    if let Err(error) = result {
        eprintln!("[menu] {id}: {error}");
    }
    // The check items reflect the queue, not the click: put them back if
    // nothing changed (the next queue state updates them otherwise).
    refresh_checks(app);
}

/// Whether the player is playing, for the Play item's text.
static PLAYING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The last queue state's checks.
static CHECKS: std::sync::Mutex<Option<Checks>> = std::sync::Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq)]
struct Checks {
    shuffle: bool,
    repeat: Repeat,
    stop_after_current: bool,
    sleep: Option<SleepChoice>,
    has_current: bool,
}

/// The queue changed: the checks follow it.
pub fn queue_changed<R: Runtime>(app: &AppHandle<R>, state: &QueueState) {
    let sleep = state.sleep.map(|timer| match timer {
        SleepTimer::EndOfTrack => SleepChoice::EndOfTrack,
        SleepTimer::EndOfAlbum => SleepChoice::EndOfAlbum,
        SleepTimer::At { minutes, .. } => SleepChoice::Minutes(minutes),
    });
    let checks = Checks {
        shuffle: state.shuffle,
        repeat: state.repeat,
        stop_after_current: state.stop_after.is_some()
            && state.stop_after == state.current_item.as_ref().map(|item| item.uid),
        sleep,
        has_current: state.current_item.is_some(),
    };
    *CHECKS.lock().unwrap_or_else(|e| e.into_inner()) = Some(checks);
    refresh_checks(app);
}

/// The player started or stopped playing: "Play" or "Pause".
pub fn playing_changed<R: Runtime>(app: &AppHandle<R>, playing: bool) {
    PLAYING.store(playing, std::sync::atomic::Ordering::Relaxed);
    if let Some(state) = app.try_state::<MenuState<R>>() {
        let _ = state.play.set_text(if playing { "Pause" } else { "Play" });
    }
}

fn refresh_checks<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<MenuState<R>>() else {
        return;
    };
    let Some(checks) = *CHECKS.lock().unwrap_or_else(|e| e.into_inner()) else {
        return;
    };
    let _ = state.shuffle.set_checked(checks.shuffle);
    for (check, repeat) in state
        .repeat
        .iter()
        .zip([Repeat::Off, Repeat::All, Repeat::One])
    {
        let _ = check.set_checked(checks.repeat == repeat);
    }
    let _ = state.stop_after.set_checked(checks.stop_after_current);
    let _ = state.stop_after.set_enabled(checks.has_current);
    for (choice, check) in &state.sleep {
        let _ = check.set_checked(*choice == checks.sleep);
    }
}

/// What the shortcuts sheet lists from the menus: (item, shortcut).
pub fn shortcuts() -> Vec<(&'static str, &'static str)> {
    let mut list: Vec<(&str, &str)> = vec![
        ("Play or pause", "Space"),
        ("Next", "CmdOrCtrl+Right"),
        ("Previous", "CmdOrCtrl+Left"),
        ("Increase volume", "CmdOrCtrl+Up"),
        ("Decrease volume", "CmdOrCtrl+Down"),
        ("Stop after this track", "Alt+CmdOrCtrl+Period"),
    ];
    list.extend(
        PAGE_ITEMS.iter().filter_map(|(_, text, accelerator)| {
            accelerator.map(|accelerator| (*text, accelerator))
        }),
    );
    list
}
