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
    let mut unique = Vec::new();
    for command in preferences.quick_access.commands {
        if !unique.contains(&command) {
            unique.push(command);
        }
    }
    preferences.quick_access.commands = unique;
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

pub fn quick_access() -> QuickAccess {
    read_preferences().quick_access
}

pub fn save_quick_access(quick_access: &QuickAccess) -> Result<(), String> {
    let mut preferences = read_preferences();
    preferences.quick_access = quick_access.clone();
    write_preferences(&preferences)
}
