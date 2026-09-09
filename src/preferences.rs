use crate::document::Color;
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_PREFERENCES_BYTES: usize = 1024 * 1024;
pub const CUSTOM_COLOR_COUNT: usize = 16;
pub const RECENT_CUSTOM_COLOR_COUNT: usize = 10;
#[cfg(target_arch = "wasm32")]
const BROWSER_STORAGE_KEY: &str = "paint-10.preferences.v1";

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    #[cfg_attr(target_arch = "wasm32", serde(skip))]
    recent_files: Vec<PathBuf>,
    custom_colors: Vec<Color>,
    recent_custom_colors: Option<Vec<Color>>,
    quick_access: QuickAccess,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuickCommand {
    New,
    Open,
    Save,
    Undo,
    Redo,
    PrintPreview,
    Print,
    Email,
}

impl QuickCommand {
    pub const ALL: [Self; 8] = [
        Self::New,
        Self::Open,
        Self::Save,
        Self::Undo,
        Self::Redo,
        Self::PrintPreview,
        Self::Print,
        Self::Email,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::Open => "Open",
            Self::Save => "Save",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::PrintPreview => "Print preview",
            Self::Print => "Print",
            Self::Email => "Send in email",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuickAccess {
    pub commands: Vec<QuickCommand>,
    pub below_ribbon: bool,
}

impl Default for QuickAccess {
    fn default() -> Self {
        Self {
            commands: vec![QuickCommand::Save, QuickCommand::Undo, QuickCommand::Redo],
            below_ribbon: false,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn settings_path() -> Option<PathBuf> {
    let directory = settings_directory(std::env::consts::OS, |name| std::env::var_os(name))?;
    Some(directory.join("paint-10/preferences.json"))
}

#[cfg(not(target_arch = "wasm32"))]
fn settings_directory(
    platform: &str,
    variable: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let absolute = |name: &str| {
        variable(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    if let Some(directory) = absolute("XDG_CONFIG_HOME") {
        return Some(directory);
    }
    match platform {
        "windows" => absolute("APPDATA")
            .or_else(|| absolute("USERPROFILE").map(|home| home.join("AppData/Roaming"))),
        "macos" => absolute("HOME").map(|home| home.join("Library/Application Support")),
        _ => absolute("HOME").map(|home| home.join(".config")),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_preferences() -> Preferences {
    let Some(path) = settings_path() else {
        return Preferences::default();
    };
    let Ok(file) = std::fs::File::open(path) else {
        return Preferences::default();
    };
    let mut bytes = Vec::new();
    if file
        .take(MAX_PREFERENCES_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Preferences::default();
    }
    decode_preferences(&bytes)
}

fn decode_preferences(bytes: &[u8]) -> Preferences {
    if bytes.len() > MAX_PREFERENCES_BYTES {
        return Preferences::default();
    }
    let mut preferences: Preferences = serde_json::from_slice(bytes).unwrap_or_default();
    preferences.recent_files.truncate(10);
    preferences.custom_colors.truncate(CUSTOM_COLOR_COUNT);
    let recent = preferences
        .recent_custom_colors
        .take()
        .unwrap_or_else(|| preferences.custom_colors.iter().rev().copied().collect());
    let mut unique_colors = Vec::new();
    for color in recent {
        if !unique_colors.contains(&color) {
            unique_colors.push(color);
        }
    }
    unique_colors.truncate(RECENT_CUSTOM_COLOR_COUNT);
    preferences.recent_custom_colors = Some(unique_colors);
    let mut unique = Vec::new();
    for command in preferences.quick_access.commands {
        if !unique.contains(&command) {
            unique.push(command);
        }
    }
    preferences.quick_access.commands = unique;
    preferences
}

fn encode_preferences(preferences: &Preferences) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec_pretty(preferences).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_PREFERENCES_BYTES {
        return Err("Preferences exceed the one-megabyte limit.".into());
    }
    Ok(bytes)
}

#[cfg(not(target_arch = "wasm32"))]
fn write_preferences(preferences: &Preferences) -> Result<(), String> {
    let path = settings_path().ok_or("No configuration directory is available.")?;
    std::fs::create_dir_all(path.parent().expect("settings path has a parent"))
        .map_err(|error| error.to_string())?;
    let bytes = encode_preferences(preferences)?;
    crate::project::atomic_write(&path, &bytes)
}

#[cfg(target_arch = "wasm32")]
fn browser_storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or("The browser window is unavailable.")?
        .local_storage()
        .map_err(|error| format!("Browser settings storage is unavailable: {error:?}"))?
        .ok_or_else(|| "Browser settings storage is unavailable.".into())
}

#[cfg(target_arch = "wasm32")]
fn read_preferences() -> Preferences {
    browser_storage()
        .ok()
        .and_then(|storage| storage.get_item(BROWSER_STORAGE_KEY).ok().flatten())
        .map_or_else(Preferences::default, |json| {
            decode_preferences(json.as_bytes())
        })
}

#[cfg(target_arch = "wasm32")]
fn write_preferences(preferences: &Preferences) -> Result<(), String> {
    let json =
        String::from_utf8(encode_preferences(preferences)?).map_err(|error| error.to_string())?;
    browser_storage()?
        .set_item(BROWSER_STORAGE_KEY, &json)
        .map_err(|error| format!("Could not save browser settings: {error:?}"))
}

pub fn recent_files() -> Vec<PathBuf> {
    read_preferences().recent_files
}

#[cfg(not(target_arch = "wasm32"))]
pub fn record_file(path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut preferences = read_preferences();
    let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
    preferences
        .recent_files
        .retain(|existing| existing != &path);
    preferences.recent_files.insert(0, path);
    preferences.recent_files.truncate(10);
    write_preferences(&preferences)?;
    Ok(preferences.recent_files)
}

#[cfg(target_arch = "wasm32")]
pub fn record_file(_path: &Path) -> Result<Vec<PathBuf>, String> {
    // A downloaded filename is not a persistent browser permission to reopen it.
    Ok(Vec::new())
}

pub fn custom_palette() -> (Vec<Color>, Vec<Color>) {
    let preferences = read_preferences();
    let mut slots = preferences.custom_colors;
    slots.resize(CUSTOM_COLOR_COUNT, crate::document::WHITE);
    (slots, preferences.recent_custom_colors.unwrap_or_default())
}

pub fn remember_custom_color(recent: &mut Vec<Color>, color: Color) {
    recent.retain(|existing| *existing != color);
    recent.insert(0, color);
    recent.truncate(RECENT_CUSTOM_COLOR_COUNT);
}

pub fn save_custom_palette(colors: &[Color], recent: &[Color]) -> Result<(), String> {
    let mut preferences = read_preferences();
    preferences.custom_colors = colors.iter().copied().take(CUSTOM_COLOR_COUNT).collect();
    preferences
        .custom_colors
        .resize(CUSTOM_COLOR_COUNT, crate::document::WHITE);
    preferences.recent_custom_colors = Some(
        recent
            .iter()
            .copied()
            .take(RECENT_CUSTOM_COLOR_COUNT)
            .collect(),
    );
    write_preferences(&preferences)
}

pub fn quick_access() -> QuickAccess {
    read_preferences().quick_access
}

pub fn save_quick_access(quick_access: &QuickAccess) -> Result<(), String> {
    let mut preferences = read_preferences();
    preferences.quick_access = quick_access.clone();
    write_preferences(&preferences)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn platform_preferences_use_native_directories_and_absolute_xdg_overrides() {
        let directory = tempfile::tempdir().unwrap();
        let home = directory.path().join("user");
        let roaming = directory.path().join("roaming");
        let override_path = directory.path().join("portable-config");
        for (platform, expected) in [
            ("linux", home.join(".config")),
            ("macos", home.join("Library/Application Support")),
            ("windows", roaming.clone()),
        ] {
            let variables = |name: &str| match name {
                "HOME" | "USERPROFILE" => Some(home.clone().into_os_string()),
                "APPDATA" => Some(roaming.clone().into_os_string()),
                "XDG_CONFIG_HOME" => Some("relative/ignored".into()),
                _ => None,
            };
            assert_eq!(settings_directory(platform, variables), Some(expected));
            assert_eq!(
                settings_directory(platform, |name| {
                    if name == "XDG_CONFIG_HOME" {
                        Some(override_path.clone().into_os_string())
                    } else {
                        variables(name)
                    }
                }),
                Some(override_path.clone())
            );
        }
        assert_eq!(
            settings_directory("windows", |name| {
                (name == "USERPROFILE").then(|| home.clone().into_os_string())
            }),
            Some(home.join("AppData/Roaming"))
        );
        assert!(settings_directory("linux", |_| None).is_none());
        assert!(settings_directory("windows", |_| Some("relative".into())).is_none());
    }

    #[test]
    fn bounded_preferences_preserve_colors_and_toolbar_without_duplicate_commands() {
        let preferences = Preferences {
            custom_colors: vec![[12, 34, 56, 78]; 20],
            quick_access: QuickAccess {
                commands: vec![QuickCommand::Save, QuickCommand::Undo, QuickCommand::Save],
                below_ribbon: true,
            },
            ..Default::default()
        };
        let decoded = decode_preferences(&encode_preferences(&preferences).unwrap());
        assert_eq!(
            decoded.custom_colors,
            vec![[12, 34, 56, 78]; CUSTOM_COLOR_COUNT]
        );
        assert_eq!(
            decoded.quick_access.commands,
            vec![QuickCommand::Save, QuickCommand::Undo]
        );
        assert!(decoded.quick_access.below_ribbon);
        let oversized = vec![b' '; MAX_PREFERENCES_BYTES + 1];
        assert!(decode_preferences(&oversized).custom_colors.is_empty());
        assert_eq!(
            decode_preferences(b"invalid").quick_access.commands,
            QuickAccess::default().commands
        );
    }

    #[test]
    fn custom_slots_and_recent_colors_round_trip_without_overwriting_legacy_slots() {
        let slots: Vec<_> = (0..CUSTOM_COLOR_COUNT)
            .map(|index| [index as u8, 40, 80, 100 + index as u8])
            .collect();
        let mut recent = Vec::new();
        for color in &slots {
            remember_custom_color(&mut recent, *color);
        }
        assert_eq!(recent.len(), RECENT_CUSTOM_COLOR_COUNT);
        assert_eq!(recent[0], slots[15]);
        remember_custom_color(&mut recent, slots[8]);
        assert_eq!(recent[0], slots[8]);
        assert_eq!(recent.iter().filter(|color| **color == slots[8]).count(), 1);

        let preferences = Preferences {
            custom_colors: slots.clone(),
            recent_custom_colors: Some(recent.clone()),
            ..Default::default()
        };
        let decoded = decode_preferences(&encode_preferences(&preferences).unwrap());
        assert_eq!(decoded.custom_colors, slots);
        assert_eq!(decoded.recent_custom_colors, Some(recent));

        let legacy =
            serde_json::to_vec(&serde_json::json!({ "custom_colors": &slots[..10] })).unwrap();
        let decoded = decode_preferences(&legacy);
        assert_eq!(decoded.custom_colors, slots[..10]);
        assert_eq!(
            decoded.recent_custom_colors.unwrap(),
            slots[..10].iter().rev().copied().collect::<Vec<_>>()
        );
    }
}
