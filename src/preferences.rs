use crate::document::Color;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    recent_files: Vec<PathBuf>,
    custom_colors: Vec<Color>,
}

fn settings_path() -> Option<PathBuf> {
    let directory = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(directory.join("paint-10/preferences.json"))
}

fn read_preferences() -> Preferences {
    let Some(path) = settings_path() else {
        return Preferences::default();
    };
    let Ok(file) = std::fs::File::open(path) else {
        return Preferences::default();
    };
    let mut bytes = Vec::new();
    if file.take(1024 * 1024).read_to_end(&mut bytes).is_err() {
        return Preferences::default();
    }
    let mut preferences: Preferences = serde_json::from_slice(&bytes).unwrap_or_default();
    preferences.recent_files.truncate(10);
    preferences.custom_colors.truncate(10);
    preferences
}

fn write_preferences(preferences: &Preferences) -> Result<(), String> {
    let path = settings_path().ok_or("No configuration directory is available.")?;
    std::fs::create_dir_all(path.parent().expect("settings path has a parent"))
        .map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec_pretty(preferences).map_err(|error| error.to_string())?;
    crate::project::atomic_write(&path, &bytes)
}

pub fn recent_files() -> Vec<PathBuf> {
    read_preferences().recent_files
}

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

pub fn custom_colors() -> Vec<Color> {
    read_preferences().custom_colors
}

pub fn save_custom_colors(colors: &[Color]) -> Result<(), String> {
    let mut preferences = read_preferences();
    preferences.custom_colors = colors.iter().copied().take(10).collect();
    write_preferences(&preferences)
}
