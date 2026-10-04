//! Themes (PLAN.md X1): the look the UI takes, kept in `AppSettings` as
//! token values (colours, a font from a fixed list, sizes in range), never
//! as CSS, so the webview's CSP and `validate` keep their meaning. The
//! frontend turns them into custom properties on `:root`
//! (`app/src/lib/theme.ts`). The built-in themes are `app/src/lib/themes.json`,
//! read here too: the first is the default.
//!
//! Themes export to a JSON file and import from one, read as leniently as
//! stored settings.

use std::path::PathBuf;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Runtime};

use crate::coded::coded;

/// The look of the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    /// The theme in use.
    pub theme: Theme,
    /// The user's themes, by name, at most `MAX_SAVED`.
    pub saved: Vec<Theme>,
    /// Uses the high-contrast theme's colours while the system asks for
    /// more contrast (F18).
    pub follow_system_contrast: bool,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        AppearanceSettings {
            theme: Theme::default(),
            saved: Vec::new(),
            follow_system_contrast: true,
        }
    }
}

/// The most themes the user can keep.
pub const MAX_SAVED: usize = 50;

/// One theme's tokens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Theme {
    /// What the user called it; the UI names built-in ones itself.
    pub name: String,
    /// The built-in theme these came from ("system", "dark"…), or "custom".
    pub preset: String,
    /// Which palette shows: the system's choice, or always one.
    pub scheme: Scheme,
    pub light: ThemePalette,
    pub dark: ThemePalette,
    pub font: Font,
    /// The base text size in px, 12 to 22; everything sized in rem follows.
    pub text_size: u32,
    pub density: Density,
    /// Corner radius in px, 0 to 16.
    pub radius: u32,
    /// The accent takes the current cover's main colour, made readable.
    pub accent_from_cover: bool,
}

/// The text size range, in px.
pub const TEXT_SIZES: std::ops::RangeInclusive<u32> = 12..=22;
/// The corner radius range, in px.
pub const RADII: std::ops::RangeInclusive<u32> = 0..=16;

static BUILT_IN: LazyLock<Vec<Theme>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../src/lib/themes.json")).expect("themes.json is valid")
});

impl Default for Theme {
    fn default() -> Self {
        BUILT_IN[0].clone()
    }
}

/// A theme's colours, each `#rrggbb`. The names are the CSS custom
/// properties', in camel case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ThemePalette {
    pub bg: String,
    pub surface: String,
    /// Raised controls, and hover.
    pub surface2: String,
    pub text: String,
    pub text_muted: String,
    /// Icons and separators, never text.
    pub text_faint: String,
    pub border: String,
    pub accent: String,
    /// Text on the accent.
    pub accent_text: String,
    pub danger: String,
    pub heart: String,
    pub star: String,
}

impl ThemePalette {
    fn colors(&self) -> [(&'static str, &str); 12] {
        [
            ("bg", &self.bg),
            ("surface", &self.surface),
            ("surface2", &self.surface2),
            ("text", &self.text),
            ("textMuted", &self.text_muted),
            ("textFaint", &self.text_faint),
            ("border", &self.border),
            ("accent", &self.accent),
            ("accentText", &self.accent_text),
            ("danger", &self.danger),
            ("heart", &self.heart),
            ("star", &self.star),
        ]
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Scheme {
    /// Light or dark as the system is.
    #[default]
    System,
    Light,
    Dark,
}

/// The fonts a theme can use; the frontend maps each to a font stack.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Font {
    #[default]
    System,
    Rounded,
    Serif,
    Mono,
    Humanist,
}

/// How much room lists and controls take.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum Density {
    Compact,
    #[default]
    Regular,
    Roomy,
}

fn is_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_name(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 64
}

impl Theme {
    /// Checks every token, naming the first bad one.
    pub fn validate(&self) -> Result<(), String> {
        if !is_name(&self.name) {
            return Err("A theme's name must be 1 to 64 bytes".into());
        }
        if !is_name(&self.preset) {
            return Err("A theme's preset must be 1 to 64 bytes".into());
        }
        for palette in [&self.light, &self.dark] {
            if let Some((token, _)) = palette.colors().into_iter().find(|(_, c)| !is_color(c)) {
                return Err(format!("The theme's {token} colour must be #rrggbb"));
            }
        }
        if !TEXT_SIZES.contains(&self.text_size) {
            return Err("The text size must be 12 to 22 px".into());
        }
        if !RADII.contains(&self.radius) {
            return Err("The corner radius must be 0 to 16 px".into());
        }
        Ok(())
    }
}

impl AppearanceSettings {
    pub fn validate(&self) -> Result<(), String> {
        self.theme.validate()?;
        if self.saved.len() > MAX_SAVED {
            return Err(format!("At most {MAX_SAVED} themes can be kept"));
        }
        let mut names = std::collections::HashSet::new();
        for theme in &self.saved {
            theme.validate()?;
            if !names.insert(theme.name.trim().to_lowercase()) {
                return Err("Two saved themes have the same name".into());
            }
        }
        Ok(())
    }
}

// ---- Files --------------------------------------------------------------------

/// What a theme file says it is.
const FORMAT: &str = "ano-mp theme";
/// The file format's version; a newer one is read as far as it can be.
const VERSION: u32 = 1;
/// No theme file is anywhere near this.
const MAX_FILE_BYTES: u64 = 256 * 1024;

/// A theme file's contents.
fn to_file(theme: &Theme) -> Value {
    json!({ "format": FORMAT, "version": VERSION, "theme": theme })
}

/// The theme in a file's contents, keeping each usable token and the
/// default for the rest (a newer version's font, say); `None` if it isn't
/// a theme file.
fn from_file(json: &str) -> Option<Theme> {
    let file: Value = serde_json::from_str(json).ok()?;
    if file["format"] != FORMAT || !file["theme"].is_object() {
        return None;
    }
    let settings = crate::settings::from_value(&json!({
        "appearance": { "theme": file["theme"] }
    }));
    Some(settings.appearance.theme)
}

/// Refuses while themes are turned off (`FeatureSettings::themes`).
fn check_on<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if crate::settings::current(app).features.themes {
        Ok(())
    } else {
        Err(crate::coded::feature_off("themes", "Themes"))
    }
}

fn not_a_theme() -> String {
    coded("notATheme", &[], "The file isn't an ano-mp theme")
}

/// Writes `theme` to the file the user named.
#[tauri::command]
pub async fn theme_export<R: Runtime>(
    app: AppHandle<R>,
    path: PathBuf,
    theme: Theme,
) -> Result<(), String> {
    check_on(&app)?;
    theme.validate()?;
    let json = serde_json::to_vec_pretty(&to_file(&theme)).map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || std::fs::write(&path, json))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("Cannot write the file: {e}"))
}

/// Reads the theme in the file the user picked; it isn't applied or saved.
#[tauri::command]
pub async fn theme_import<R: Runtime>(app: AppHandle<R>, path: PathBuf) -> Result<Theme, String> {
    check_on(&app)?;
    let json = tauri::async_runtime::spawn_blocking(move || {
        let size = std::fs::metadata(&path)?.len();
        if size > MAX_FILE_BYTES {
            return Ok(None);
        }
        std::fs::read_to_string(&path).map(Some)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("Cannot read the file: {e}"))?;
    json.as_deref().and_then(from_file).ok_or_else(not_a_theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_themes_are_valid_and_distinct() {
        assert_eq!(Theme::default().preset, "system");
        let mut presets = std::collections::HashSet::new();
        for theme in BUILT_IN.iter() {
            theme.validate().unwrap();
            assert!(presets.insert(theme.preset.clone()), "{}", theme.preset);
        }
        AppearanceSettings::default().validate().unwrap();
    }

    #[test]
    fn refuses_bad_tokens() {
        let error = |edit: fn(&mut Theme)| {
            let mut theme = Theme::default();
            edit(&mut theme);
            theme.validate().unwrap_err()
        };
        assert!(error(|t| t.light.accent = "blue".into()).contains("accent"));
        assert!(error(|t| t.dark.bg = "#12345".into()).contains("bg"));
        assert!(error(|t| t.dark.text = "#12345g".into()).contains("text"));
        assert!(error(|t| t.light.surface2 = "url(x)".into()).contains("surface2"));
        assert!(error(|t| t.text_size = 30).contains("text size"));
        assert!(error(|t| t.radius = 17).contains("radius"));
        assert!(error(|t| t.name = " ".into()).contains("name"));

        let appearance = AppearanceSettings {
            saved: vec![Theme::default(), Theme::default()],
            ..AppearanceSettings::default()
        };
        assert!(appearance.validate().unwrap_err().contains("same name"));
    }

    #[test]
    fn a_file_round_trips_and_is_read_leniently() {
        let mut theme = BUILT_IN[4].clone();
        theme.name = "Mine ♪".into();
        theme.preset = "custom".into();
        let json = to_file(&theme).to_string();
        assert_eq!(from_file(&json), Some(theme.clone()));

        // A newer version's font and a bad colour: those fall back, the rest is kept.
        let mut file = to_file(&theme);
        file["version"] = json!(7);
        file["theme"]["font"] = json!("gothic");
        file["theme"]["light"]["accent"] = json!("red");
        file["theme"]["future"] = json!(true);
        let read = from_file(&file.to_string()).unwrap();
        assert_eq!(read.font, Font::System);
        assert_eq!(read.light.accent, Theme::default().light.accent);
        assert_eq!(read.light.bg, theme.light.bg);
        assert_eq!(read.name, theme.name);

        for other in [
            "not json",
            "{}",
            r#"{"format": "ano-mp", "theme": {}}"#,
            r#"{"format": "ano-mp theme", "theme": 3}"#,
        ] {
            assert_eq!(from_file(other), None, "{other}");
        }
    }
}
