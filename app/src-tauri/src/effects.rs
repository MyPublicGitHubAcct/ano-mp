//! Real-time effects on what is playing (PLAN.md X2): the settings for the
//! core's effect chain (the `anomp_effects` library, through `anomp.h`),
//! applying them to the engine, and the commands that list the effects,
//! preview a change without saving it and hold the spectral freeze.
//!
//! What the effects are, their parameters and ranges come from the core
//! (`anomp::effects`), so the settings, their validation and the UI follow
//! it. They are global (`AppSettings.effects`), behind the `effects`
//! feature switch, off by default as they change what is heard; off, every
//! effect is off whatever the settings say.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

use crate::anomp::{self, EffectInfo, Engine};
use crate::audio::with_engine;
use crate::settings;

/// Each effect's settings, by its id in the core.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EffectsSettings {
    pub reverb: EffectSettings,
    pub chorus: EffectSettings,
    pub freeze: EffectSettings,
    pub echo: EffectSettings,
    pub flanger: EffectSettings,
    pub phaser: EffectSettings,
    pub tremolo: EffectSettings,
    pub lofi: EffectSettings,
}

/// One effect: on or off, its wet/dry mix and its parameters by id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EffectSettings {
    pub enabled: bool,
    /// 0 (the music only) to 1 (the effect only).
    pub mix: f64,
    /// Every parameter the core lists for it, within its range.
    #[cfg_attr(test, ts(type = "Record<string, number>"))]
    pub params: BTreeMap<String, f64>,
}

static CATALOG: LazyLock<Vec<EffectInfo>> = LazyLock::new(anomp::effects);

/// The core's effects, by number.
pub fn catalog() -> &'static [EffectInfo] {
    &CATALOG
}

impl EffectSettings {
    /// Off, with the core's defaults.
    fn defaults(info: &EffectInfo) -> EffectSettings {
        EffectSettings {
            enabled: false,
            mix: info.default_mix,
            params: info
                .params
                .iter()
                .map(|param| (param.id.clone(), param.default_value))
                .collect(),
        }
    }
}

impl Default for EffectsSettings {
    fn default() -> Self {
        let of = |id: &str| {
            catalog()
                .iter()
                .find(|info| info.id == id)
                .map(EffectSettings::defaults)
                .unwrap_or_default()
        };
        EffectsSettings {
            reverb: of("reverb"),
            chorus: of("chorus"),
            freeze: of("freeze"),
            echo: of("echo"),
            flanger: of("flanger"),
            phaser: of("phaser"),
            tremolo: of("tremolo"),
            lofi: of("lofi"),
        }
    }
}

impl EffectsSettings {
    /// The settings of the effect the core calls `id`.
    pub fn get(&self, id: &str) -> Option<&EffectSettings> {
        Some(match id {
            "reverb" => &self.reverb,
            "chorus" => &self.chorus,
            "freeze" => &self.freeze,
            "echo" => &self.echo,
            "flanger" => &self.flanger,
            "phaser" => &self.phaser,
            "tremolo" => &self.tremolo,
            "lofi" => &self.lofi,
            _ => return None,
        })
    }

    /// Checks each effect's mix and parameters against the core's ranges.
    pub fn validate(&self) -> Result<(), String> {
        for info in catalog() {
            let Some(effect) = self.get(&info.id) else {
                return Err(format!("There are no settings for the {} effect", info.id));
            };
            if !(0.0..=1.0).contains(&effect.mix) {
                return Err(format!("The {} effect's mix must be 0 to 1", info.id));
            }
            let known = |id: &String| info.params.iter().any(|param| &param.id == id);
            if effect.params.len() != info.params.len() || !effect.params.keys().all(known) {
                let names: Vec<&str> = info.params.iter().map(|p| p.id.as_str()).collect();
                return Err(format!("The {} effect takes {}", info.id, names.join(", ")));
            }
            for param in &info.params {
                let value = effect.params[&param.id];
                if !(param.min..=param.max).contains(&value) {
                    return Err(format!(
                        "The {} effect's {} must be {} to {}",
                        info.id, param.id, param.min, param.max
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Sets the engine's effects as `settings` say, or every one off while
/// the feature is off (`on` false).
pub fn apply_to(engine: &mut Engine, settings: &EffectsSettings, on: bool) {
    for info in catalog() {
        let Some(effect) = settings.get(&info.id) else {
            continue;
        };
        let params: Vec<f64> = info
            .params
            .iter()
            .map(|param| {
                effect
                    .params
                    .get(&param.id)
                    .copied()
                    .unwrap_or(param.default_value)
            })
            .collect();
        if !engine.set_effect(info.number, on && effect.enabled, effect.mix, &params) {
            log::warn!("effect {} refused its settings", info.number);
        }
    }
}

/// The effects' settings, or the feature switch, changed.
pub fn apply<R: Runtime>(app: &AppHandle<R>, settings: &EffectsSettings, on: bool) {
    let settings = settings.clone();
    let _ = with_engine(app, move |engine| apply_to(engine, &settings, on));
}

fn require<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if settings::current(app).features.effects {
        Ok(())
    } else {
        Err(crate::coded::feature_off("effects", "Effects"))
    }
}

/// What the effects are doing now.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct EffectsStatus {
    /// The spectral freeze holds the sound; a new track lets it go.
    pub freeze_held: bool,
}

/// Every effect, its parameters and their ranges, by number.
#[tauri::command]
pub fn effects_catalog() -> Vec<EffectInfo> {
    catalog().to_vec()
}

/// Plays `effects` without saving them, while a control moves; saving the
/// settings applies them for good.
#[tauri::command]
pub fn effects_preview<R: Runtime>(
    app: AppHandle<R>,
    effects: EffectsSettings,
) -> Result<(), String> {
    require(&app)?;
    effects.validate()?;
    with_engine(&app, move |engine| apply_to(engine, &effects, true))
}

/// Holds the spectral freeze's sound, or lets it go; returns whether it
/// holds (it needs the freeze on).
#[tauri::command]
pub fn effects_freeze<R: Runtime>(app: AppHandle<R>, hold: bool) -> Result<bool, String> {
    if hold {
        require(&app)?;
    }
    with_engine(&app, move |engine| engine.set_freeze(hold))
}

#[tauri::command]
pub fn effects_status<R: Runtime>(app: AppHandle<R>) -> Result<EffectsStatus, String> {
    with_engine(&app, |engine| EffectsStatus {
        freeze_held: engine.freeze_held(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_name_every_effect_the_core_has() {
        assert!(catalog().len() >= 7, "at least seven effects");
        assert_eq!(catalog().len(), anomp::EFFECT_COUNT);
        let settings = EffectsSettings::default();
        for info in catalog() {
            let effect = settings.get(&info.id).expect("a field per effect");
            assert!(!effect.enabled, "{} is off by default", info.id);
            assert_eq!(effect.params.len(), info.params.len());
        }
        let json = serde_json::to_value(&settings).unwrap();
        assert_eq!(json.as_object().unwrap().len(), catalog().len());
        settings.validate().unwrap();
    }

    #[test]
    fn the_catalogue_reads_the_cores_ranges_as_decimals() {
        let echo = catalog().iter().find(|info| info.id == "echo").unwrap();
        let time = &echo.params[0];
        assert_eq!(time.id, "time");
        assert_eq!(time.unit, anomp::EffectUnit::Milliseconds);
        assert_eq!((time.min, time.max), (20.0, 2000.0));
        assert!(time.logarithmic);
        let chorus = catalog().iter().find(|info| info.id == "chorus").unwrap();
        assert_eq!(chorus.params[0].min, 0.05, "not 0.0500000007");
        let mut positions: Vec<u32> = catalog().iter().map(|info| info.position).collect();
        positions.sort_unstable();
        assert_eq!(positions, (0..8).collect::<Vec<u32>>());
    }

    #[test]
    fn refuses_values_out_of_range() {
        let error = |edit: fn(&mut EffectsSettings)| {
            let mut settings = EffectsSettings::default();
            edit(&mut settings);
            settings.validate().unwrap_err()
        };
        assert!(error(|s| s.reverb.mix = 1.5).contains("mix"));
        assert!(error(|s| s.chorus.mix = f64::NAN).contains("mix"));
        assert!(error(|s| {
            s.echo.params.insert("time".into(), 5000.0);
        })
        .contains("time must be 20 to 2000"));
        assert!(error(|s| {
            s.echo.params.remove("tone");
        })
        .contains("takes time, feedback, tone, spread"));
        assert!(error(|s| {
            s.lofi.params.insert("wobble".into(), 1.0);
        })
        .contains("lofi"));

        // The ends of each range are in it.
        let mut settings = EffectsSettings::default();
        for info in catalog() {
            let effect = match info.id.as_str() {
                "chorus" => &mut settings.chorus,
                "flanger" => &mut settings.flanger,
                _ => continue,
            };
            for param in &info.params {
                effect.params.insert(param.id.clone(), param.min);
            }
        }
        settings.validate().unwrap();
    }

    #[test]
    fn every_effect_and_parameter_has_a_name_in_the_ui() {
        let en: serde_json::Value =
            serde_json::from_str(include_str!("../../src/lib/i18n/en.json")).unwrap();
        for info in catalog() {
            for key in [
                format!("effects.{}", info.id),
                format!("effects.{}About", info.id),
            ] {
                assert!(en.get(&key).is_some(), "en.json lacks {key}");
            }
            for param in &info.params {
                let key = format!("effects.param.{}", param.id);
                assert!(en.get(&key).is_some(), "en.json lacks {key}");
            }
        }
    }
}
