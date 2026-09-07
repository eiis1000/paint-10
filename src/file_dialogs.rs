//! Save dialogs report both the destination and the chosen file format.
//!
//! Each platform's save flow runs on the main thread of a helper process. Linux
//! uses GTK's format selector. Windows and macOS choose the format in a small
//! eframe window before opening the native rfd destination dialog, since rfd
//! does not return the selected file filter (and BMP depths share an extension).

#[cfg(any(not(target_os = "linux"), test))]
mod format_picker;

use crate::raster_io::RasterFormat;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const HELPER_ARGUMENT: &str = "--paint-10-save-dialog";
const MAX_REQUEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveChoice {
    pub path: PathBuf,
    pub format: RasterFormat,
}

#[derive(Serialize, Deserialize)]
struct SaveRequest {
    initial_path: Option<PathBuf>,
    initial_format: RasterFormat,
    title: String,
}

/// This can be called from eframe's UI thread, regardless of prior rfd use.
pub fn save_dialog(
    initial_path: Option<&Path>,
    initial_format: RasterFormat,
) -> Result<Option<SaveChoice>, String> {
    save_dialog_with_title(initial_path, initial_format, "Save As — Paint 10")
}

pub fn save_dialog_with_title(
    initial_path: Option<&Path>,
    initial_format: RasterFormat,
    title: &str,
) -> Result<Option<SaveChoice>, String> {
    let request = SaveRequest {
        initial_path: initial_path.map(Path::to_path_buf),
        initial_format,
        title: title.to_owned(),
    };
    let bytes = serde_json::to_vec(&request).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_REQUEST_BYTES {
        return Err("The destination path is too long.".into());
    }
    let executable = helper_executable()?;
    let mut child = Command::new(executable)
        .arg(HELPER_ARGUMENT)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Could not open Save As: {error}"))?;
    let write_result = child
        .stdin
        .take()
        .ok_or("Save As input pipe is unavailable.".to_string())
        .and_then(|mut stdin| {
            stdin
                .write_all(&bytes)
                .map_err(|error| format!("Could not initialize Save As: {error}"))
        });
    if let Err(error) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr)
            .chars()
            .take(1200)
            .collect::<String>();
        return Err(format!("Save As could not open: {}", detail.trim()));
    }
    if output.stdout.len() as u64 > MAX_REQUEST_BYTES {
        return Err("Save As returned an invalid response.".into());
    }
    serde_json::from_slice::<Result<Option<SaveChoice>, String>>(&output.stdout)
        .map_err(|error| format!("Could not read the Save As result: {error}"))?
}

fn helper_executable() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    #[cfg(target_os = "linux")]
    if !executable.exists() {
        // Cargo and package upgrades can replace the binary while Paint is
        // running. The proc link still opens this process's executable inode.
        return Ok(PathBuf::from("/proc/self/exe"));
    }
    Ok(executable)
}

/// Call before starting eframe. Some(status) means the helper request was handled.
pub fn run_helper_if_requested() -> Option<i32> {
    if std::env::args_os().nth(1).as_deref() != Some(std::ffi::OsStr::new(HELPER_ARGUMENT)) {
        return None;
    }
    let response = (|| {
        let mut bytes = Vec::new();
        std::io::stdin()
            .take(MAX_REQUEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_REQUEST_BYTES {
            return Err("Save As request is too large.".into());
        }
        let request: SaveRequest =
            serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        native_save_dialog(request)
    })();
    Some(
        if serde_json::to_writer(std::io::stdout().lock(), &response).is_ok() {
            0
        } else {
            1
        },
    )
}

fn suggested_filename(name: &str, format: RasterFormat) -> String {
    let path = Path::new(name);
    if format.matches_path(path) {
        return name.to_owned();
    }
    if path.extension().is_none() || RasterFormat::from_path(path).is_some() {
        path.with_extension(format.extensions()[0])
            .to_string_lossy()
            .into_owned()
    } else {
        format!("{name}.{}", format.extensions()[0])
    }
}

fn resolve_choice(path: PathBuf, selected: RasterFormat) -> SaveChoice {
    // Respect an explicitly typed supported extension, while preserving the
    // selected BMP bit depth when every BMP filter shares the same extension.
    let format = if selected.matches_path(&path) {
        selected
    } else {
        RasterFormat::from_path(&path).unwrap_or(selected)
    };
    let path = if format.matches_path(&path) {
        path
    } else {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Untitled");
        path.with_file_name(suggested_filename(name, format))
    };
    SaveChoice { path, format }
}

#[cfg(target_os = "linux")]
fn native_save_dialog(request: SaveRequest) -> Result<Option<SaveChoice>, String> {
    use gtk::prelude::*;
    gtk::init().map_err(|error| format!("Could not initialize the file dialog: {error}"))?;
    let dialog = gtk::FileChooserDialog::with_buttons(
        Some(&request.title),
        None::<&gtk::Window>,
        gtk::FileChooserAction::Save,
        &[
            ("Cancel", gtk::ResponseType::Cancel),
            ("Save", gtk::ResponseType::Accept),
        ],
    );
    dialog.set_default_response(gtk::ResponseType::Accept);
    dialog.set_modal(true);
    dialog.set_do_overwrite_confirmation(true);
    dialog.set_default_size(800, 560);
    let mut filters = Vec::new();
    for format in RasterFormat::ALL {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(&format!(
            "{} ({})",
            format.label(),
            format
                .extensions()
                .iter()
                .map(|extension| format!("*.{extension}"))
                .collect::<Vec<_>>()
                .join(", ")
        )));
        for extension in format.extensions() {
            filter.add_pattern(&format!("*.{extension}"));
            filter.add_pattern(&format!("*.{}", extension.to_ascii_uppercase()));
        }
        dialog.add_filter(filter.clone());
        if format == request.initial_format {
            dialog.set_filter(&filter);
        }
        filters.push((filter, format));
    }
    if let Some(parent) = request.initial_path.as_ref().and_then(|path| path.parent()) {
        if !parent.as_os_str().is_empty() {
            dialog.set_current_folder(parent);
        }
    }
    let initial_name = request
        .initial_path
        .as_ref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled");
    dialog.set_current_name(&suggested_filename(initial_name, request.initial_format));
    let changed_filters = filters.clone();
    dialog.connect_filter_notify(move |dialog| {
        if let Some((_, format)) = changed_filters
            .iter()
            .find(|(filter, _)| Some(filter) == dialog.filter().as_ref())
        {
            let name = dialog
                .current_name()
                .map(|name| name.to_string())
                .unwrap_or_else(|| "Untitled".into());
            let updated = suggested_filename(&name, *format);
            if updated != name {
                dialog.set_current_name(&updated);
            }
        }
    });
    let result = loop {
        if dialog.run() != gtk::ResponseType::Accept {
            break None;
        }
        let Some(path) = dialog.filename() else {
            continue;
        };
        let selected = filters
            .iter()
            .find(|(filter, _)| Some(filter) == dialog.filter().as_ref())
            .map(|(_, format)| *format)
            .unwrap_or(request.initial_format);
        let choice = resolve_choice(path.clone(), selected);
        // GTK already confirmed the typed path. If we appended an extension,
        // confirm that actual destination too before allowing an overwrite.
        if choice.path != path && choice.path.exists() {
            let confirm = gtk::MessageDialog::builder()
                .transient_for(&dialog)
                .modal(true)
                .message_type(gtk::MessageType::Question)
                .buttons(gtk::ButtonsType::YesNo)
                .text(format!(
                    "Replace {}?",
                    choice
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                ))
                .secondary_text("A file with this name already exists.")
                .build();
            let replace = confirm.run() == gtk::ResponseType::Yes;
            confirm.close();
            if !replace {
                continue;
            }
        }
        break Some(choice);
    };
    dialog.close();
    Ok(result)
}

#[cfg(not(target_os = "linux"))]
fn native_save_dialog(request: SaveRequest) -> Result<Option<SaveChoice>, String> {
    let Some(format) = format_picker::choose(&request.title, request.initial_format)? else {
        return Ok(None);
    };
    let mut dialog = rfd::FileDialog::new()
        .set_title(&request.title)
        .add_filter(format.label(), format.extensions());
    if let Some(path) = request.initial_path.as_deref() {
        if let Some(parent) = path.parent() {
            dialog = dialog.set_directory(parent);
        }
        dialog = dialog.set_file_name(suggested_filename(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Untitled"),
            format,
        ));
    } else {
        dialog = dialog.set_file_name(suggested_filename("Untitled", format));
    }
    Ok(dialog.save_file().map(|path| resolve_choice(path, format)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changing_format_updates_the_extension_and_preserves_basename() {
        assert_eq!(
            suggested_filename("Untitled.png", RasterFormat::Jpeg),
            "Untitled.jpg"
        );
        assert_eq!(
            suggested_filename("draft.v1", RasterFormat::Png),
            "draft.v1.png"
        );
        assert_eq!(
            suggested_filename("picture.dib", RasterFormat::BmpMono),
            "picture.dib"
        );
        assert_eq!(
            suggested_filename("picture.jpg", RasterFormat::Project),
            "picture.p10"
        );
    }

    #[test]
    fn selected_bitmap_depth_survives_shared_extensions() {
        let choice = resolve_choice(PathBuf::from("picture.bmp"), RasterFormat::Bmp16);
        assert_eq!(choice.format, RasterFormat::Bmp16);
        let choice = resolve_choice(PathBuf::from("picture"), RasterFormat::BmpMono);
        assert_eq!(choice.path, PathBuf::from("picture.bmp"));
        assert_eq!(choice.format, RasterFormat::BmpMono);
        let choice = resolve_choice(PathBuf::from("editable.p10"), RasterFormat::Png);
        assert_eq!(choice.format, RasterFormat::Project);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn helper_reexecutes_after_its_running_executable_is_deleted() {
        const TEST: &str =
            "file_dialogs::tests::helper_reexecutes_after_its_running_executable_is_deleted";
        const FIXTURE: &str = "PAINT_10_TEST_DELETED_EXECUTABLE";
        const PROBE: &str = "PAINT_10_TEST_HELPER_PROBE";
        const NAME: &str = "paint10-deleted-helper-fixture";
        if std::env::var_os(PROBE).is_some() {
            assert_eq!(
                helper_executable().unwrap(),
                PathBuf::from("/proc/self/exe")
            );
            return;
        }
        if let Some(path) = std::env::var_os(FIXTURE).map(PathBuf::from) {
            assert_eq!(path.file_name(), Some(std::ffi::OsStr::new(NAME)));
            assert_eq!(std::env::current_exe().unwrap(), path);
            std::fs::remove_file(&path).unwrap();
            assert!(!path.exists());
            let output = Command::new(helper_executable().unwrap())
                .args(["--exact", TEST])
                .env_remove(FIXTURE)
                .env(PROBE, "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let executable = std::env::current_exe().unwrap();
        assert_eq!(helper_executable().unwrap(), executable);
        let directory = tempfile::tempdir().unwrap();
        let copy = directory.path().join(NAME);
        std::fs::copy(executable, &copy).unwrap();
        let output = Command::new(&copy)
            .args(["--exact", TEST])
            .env(FIXTURE, &copy)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!copy.exists());
    }
}
