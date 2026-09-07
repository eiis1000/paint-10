//! Desktop operations. Call these blocking functions on a worker thread.
//!
//! Captures require an explicit device selection. Opening the import dialog only
//! enumerates devices; it never takes a photograph or starts a scan. Linux uses
//! SANE's `scanimage`, FFmpeg's V4L2 input, and the desktop portals. No command is
//! interpreted by a shell, and the email portal only opens a compose window.

#[cfg(any(target_os = "linux", test))]
use image::ImageFormat;
use image::{imageops, Rgba, RgbaImage};
#[cfg(any(target_os = "linux", test))]
use std::io::Cursor;
#[cfg(target_os = "linux")]
use std::io::{Seek, SeekFrom};
use std::sync::atomic::AtomicBool;
#[cfg(any(target_os = "linux", all(test, unix)))]
use std::{
    io::Read,
    process::{Command, Stdio},
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};

const MAX_PIXELS: u64 = 16_777_216;
#[cfg(any(target_os = "linux", test))]
const MAX_CAPTURE_BYTES: usize = 96 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Scanner,
    Camera,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureDevice {
    pub kind: DeviceKind,
    pub id: String,
    pub name: String,
}

#[derive(Default, Debug)]
pub struct DeviceList {
    pub devices: Vec<CaptureDevice>,
    /// Backends unavailable on this machine, without hiding working backends.
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScanMode {
    #[default]
    DeviceDefault,
    Color,
    Gray,
    Lineart,
}

#[derive(Clone, Debug)]
pub struct CaptureSettings {
    /// None keeps the scanner's current resolution.
    pub resolution_dpi: Option<u32>,
    pub scan_mode: ScanMode,
    /// None keeps the camera's current supported frame size.
    pub camera_size: Option<(u32, u32)>,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            resolution_dpi: Some(150),
            scan_mode: ScanMode::DeviceDefault,
            camera_size: None,
        }
    }
}

/// Enumerates SANE scanners and Linux video nodes without capturing anything.
/// Scanner discovery can take up to 15 seconds; run it outside the GUI thread.
#[cfg(target_os = "linux")]
pub fn enumerate_devices() -> Result<DeviceList, String> {
    let mut result = DeviceList::default();
    let mut scanner = Command::new("scanimage");
    scanner.arg("--formatted-device-list=%d\t%v %m\t%t%n");
    match run_tool(
        &mut scanner,
        "Scanner discovery",
        1024 * 1024,
        Duration::from_secs(15),
        &AtomicBool::new(false),
    ) {
        Ok(bytes) => result
            .devices
            .extend(parse_scanners(&String::from_utf8_lossy(&bytes))),
        Err(error) => result.warnings.push(error),
    }
    match std::fs::read_dir("/sys/class/video4linux") {
        Ok(entries) => {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !is_video_node(&name) {
                    continue;
                }
                let label = std::fs::File::open(entry.path().join("name"))
                    .and_then(|file| {
                        let mut text = String::new();
                        file.take(4096).read_to_string(&mut text)?;
                        Ok(text)
                    })
                    .unwrap_or_else(|_| name.clone());
                result.devices.push(CaptureDevice {
                    kind: DeviceKind::Camera,
                    id: format!("/dev/{name}"),
                    name: format!("{} ({name})", label.trim()),
                });
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => result
            .warnings
            .push(format!("Could not list cameras: {error}")),
    }
    result.devices.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

#[cfg(not(target_os = "linux"))]
pub fn enumerate_devices() -> Result<DeviceList, String> {
    Err("Scanner and camera capture currently requires Linux. You can open a picture saved by your device's application.".into())
}

#[cfg(any(target_os = "linux", test))]
fn parse_scanners(text: &str) -> Vec<CaptureDevice> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.splitn(3, '\t');
            let id = fields.next()?.trim();
            let name = fields.next()?.trim();
            if id.is_empty() || id.chars().any(char::is_control) {
                return None;
            }
            Some(CaptureDevice {
                kind: DeviceKind::Scanner,
                id: id.into(),
                name: if name.is_empty() { id } else { name }.into(),
            })
        })
        .collect()
}

#[cfg(any(target_os = "linux", test))]
fn is_video_node(name: &str) -> bool {
    name.strip_prefix("video")
        .is_some_and(|suffix| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit()))
}

/// Captures one picture after the user chooses Import/Capture. Setting `cancel`
/// terminates our capture subprocess and discards the incomplete image.
#[cfg(target_os = "linux")]
pub fn capture(
    device: &CaptureDevice,
    settings: &CaptureSettings,
    cancel: &AtomicBool,
) -> Result<RgbaImage, String> {
    use std::os::unix::fs::FileTypeExt;
    if cancel.load(Ordering::Relaxed) {
        return Err("Capture canceled.".into());
    }
    let (mut command, label, timeout) = match device.kind {
        DeviceKind::Scanner => {
            if device.id.is_empty() || device.id.chars().any(char::is_control) {
                return Err("Choose a valid scanner.".into());
            }
            let mut command = Command::new("scanimage");
            command
                .arg(format!("--device-name={}", device.id))
                .arg("--format=png");
            if let Some(dpi) = settings.resolution_dpi {
                if !(75..=600).contains(&dpi) {
                    return Err("Choose a scan resolution between 75 and 600 DPI.".into());
                }
                command.arg(format!("--resolution={dpi}"));
            }
            match settings.scan_mode {
                ScanMode::DeviceDefault => {}
                ScanMode::Color => {
                    command.arg("--mode=Color");
                }
                ScanMode::Gray => {
                    command.arg("--mode=Gray");
                }
                ScanMode::Lineart => {
                    command.arg("--mode=Lineart");
                }
            }
            (command, "Scan", Duration::from_secs(180))
        }
        DeviceKind::Camera => {
            let node = device
                .id
                .strip_prefix("/dev/")
                .filter(|name| is_video_node(name))
                .ok_or("Choose a valid camera device.")?;
            let path = format!("/dev/{node}");
            let metadata =
                std::fs::metadata(&path).map_err(|e| format!("Camera is unavailable: {e}"))?;
            if !metadata.file_type().is_char_device() {
                return Err("The selected camera is not a video device.".into());
            }
            let mut command = Command::new("ffmpeg");
            command.args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-f",
                "video4linux2",
            ]);
            if let Some((width, height)) = settings.camera_size {
                check_size(width, height)?;
                command.args(["-video_size", &format!("{width}x{height}")]);
            }
            command.arg("-i").arg(path).args([
                "-frames:v",
                "1",
                "-threads",
                "1",
                "-f",
                "image2pipe",
                "-c:v",
                "png",
                "pipe:1",
            ]);
            (command, "Camera capture", Duration::from_secs(30))
        }
    };
    let bytes = run_tool(&mut command, label, MAX_CAPTURE_BYTES, timeout, cancel)?;
    if cancel.load(Ordering::Relaxed) {
        return Err("Capture canceled.".into());
    }
    decode_capture(&bytes)
}

#[cfg(not(target_os = "linux"))]
pub fn capture(
    _: &CaptureDevice,
    _: &CaptureSettings,
    _: &AtomicBool,
) -> Result<RgbaImage, String> {
    Err("Scanner and camera capture currently requires Linux.".into())
}

fn check_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > 16384
        || height > 16384
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        Err("The picture exceeds the 16 megapixel limit. Choose a lower capture resolution.".into())
    } else {
        Ok(())
    }
}

#[cfg(any(target_os = "linux", test))]
fn decode_capture(bytes: &[u8]) -> Result<RgbaImage, String> {
    if bytes.len() > MAX_CAPTURE_BYTES {
        return Err("The captured picture is too large.".into());
    }
    let (width, height) = image::ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png)
        .into_dimensions()
        .map_err(|e| format!("Could not read the captured picture: {e}"))?;
    check_size(width, height)?;
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map(|image| image.to_rgba8())
        .map_err(|e| format!("Could not decode the captured picture: {e}"))
}

/// Read at most limit + 1 bytes, closing the pipe immediately on overflow.
/// Separate readers keep stderr from blocking a producer with a full pipe.
#[cfg(any(target_os = "linux", all(test, unix)))]
fn limited_read(
    reader: impl Read,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        overflow.store(true, Ordering::Relaxed);
        Err("Device output exceeded its size limit.".into())
    } else {
        Ok(bytes)
    }
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn run_tool(
    command: &mut Command,
    label: &str,
    limit: usize,
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<Vec<u8>, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            format!(
                "{label} needs {} installed and available in PATH.",
                command.get_program().to_string_lossy()
            )
        } else {
            format!("Could not start {label}: {error}")
        }
    })?;
    let overflow = Arc::new(AtomicBool::new(false));
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let out_overflow = overflow.clone();
    let err_overflow = overflow.clone();
    let output = std::thread::spawn(move || limited_read(stdout, limit, out_overflow));
    let errors = std::thread::spawn(move || limited_read(stderr, 128 * 1024, err_overflow));
    let started = Instant::now();
    let status = loop {
        let stop = if cancel.load(Ordering::Relaxed) {
            Some("Capture canceled.".to_owned())
        } else if started.elapsed() >= timeout {
            Some(format!(
                "{label} timed out. Check the device and try again."
            ))
        } else if overflow.load(Ordering::Relaxed) {
            Some(format!("{label} output exceeded its size limit."))
        } else {
            None
        };
        if let Some(reason) = stop {
            let _ = child.kill();
            let _ = child.wait();
            let _ = output.join();
            let _ = errors.join();
            return Err(reason);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = output.join();
                let _ = errors.join();
                return Err(format!("Could not wait for {label}: {error}"));
            }
        }
    };
    let bytes = output
        .join()
        .map_err(|_| "Device output reader failed.")??;
    let diagnostics = errors.join().map_err(|_| "Device error reader failed.")??;
    if !status.success() {
        let message = String::from_utf8_lossy(&diagnostics)
            .trim()
            .chars()
            .take(1500)
            .collect::<String>();
        return Err(if message.is_empty() {
            format!("{label} failed ({status}).")
        } else {
            format!("{label} failed: {message}")
        });
    }
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WallpaperStyle {
    #[default]
    Fill,
    Fit,
    Stretch,
    Tile,
    Center,
}

/// Produce the requested Paint wallpaper layout before handing it to a portal,
/// whose API does not expose fill/tile/center. Pass the target screen size.
pub fn wallpaper_image(
    image: &RgbaImage,
    style: WallpaperStyle,
    screen_size: (u32, u32),
) -> Result<RgbaImage, String> {
    check_size(image.width(), image.height())?;
    let (width, height) = screen_size;
    check_size(width, height)?;
    let mut result = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));
    match style {
        WallpaperStyle::Tile => {
            for y in (0..height).step_by(image.height() as usize) {
                for x in (0..width).step_by(image.width() as usize) {
                    imageops::overlay(&mut result, image, i64::from(x), i64::from(y));
                }
            }
        }
        WallpaperStyle::Center => imageops::overlay(
            &mut result,
            image,
            (i64::from(width) - i64::from(image.width())) / 2,
            (i64::from(height) - i64::from(image.height())) / 2,
        ),
        WallpaperStyle::Stretch => {
            let resized = imageops::resize(image, width, height, imageops::FilterType::Triangle);
            imageops::overlay(&mut result, &resized, 0, 0);
        }
        WallpaperStyle::Fill | WallpaperStyle::Fit => {
            // Crop before resizing for Fill, avoiding huge intermediate images
            // when the source and screen have very different aspect ratios.
            if style == WallpaperStyle::Fill {
                let screen_ratio = f64::from(width) / f64::from(height);
                let source_ratio = f64::from(image.width()) / f64::from(image.height());
                let (crop_w, crop_h) = if source_ratio > screen_ratio {
                    (
                        (f64::from(image.height()) * screen_ratio).round().max(1.0) as u32,
                        image.height(),
                    )
                } else {
                    (
                        image.width(),
                        (f64::from(image.width()) / screen_ratio).round().max(1.0) as u32,
                    )
                };
                let crop_w = crop_w.min(image.width());
                let crop_h = crop_h.min(image.height());
                let crop = imageops::crop_imm(
                    image,
                    (image.width() - crop_w) / 2,
                    (image.height() - crop_h) / 2,
                    crop_w,
                    crop_h,
                )
                .to_image();
                let resized =
                    imageops::resize(&crop, width, height, imageops::FilterType::Triangle);
                imageops::overlay(&mut result, &resized, 0, 0);
            } else {
                let factor = (f64::from(width) / f64::from(image.width()))
                    .min(f64::from(height) / f64::from(image.height()));
                let new_w = (f64::from(image.width()) * factor).round().max(1.0) as u32;
                let new_h = (f64::from(image.height()) * factor).round().max(1.0) as u32;
                let resized = imageops::resize(image, new_w, new_h, imageops::FilterType::Triangle);
                imageops::overlay(
                    &mut result,
                    &resized,
                    i64::from(width - new_w) / 2,
                    i64::from(height - new_h) / 2,
                );
            }
        }
    }
    Ok(result)
}

#[cfg(target_os = "linux")]
fn portal_png(image: &RgbaImage) -> Result<tempfile::NamedTempFile, String> {
    check_size(image.width(), image.height())?;
    let mut file = tempfile::Builder::new()
        .prefix("Paint 10-")
        .suffix(".png")
        .tempfile()
        .map_err(|e| e.to_string())?;
    image
        .write_to(file.as_file_mut(), ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    file.as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|e| e.to_string())?;
    Ok(file)
}

/// Shows the system wallpaper preview. The portal controls final approval.
/// Only the desktop background is requested; the lock screen is untouched.
#[cfg(target_os = "linux")]
pub fn set_wallpaper(
    image: &RgbaImage,
    style: WallpaperStyle,
    screen_size: (u32, u32),
) -> Result<(), String> {
    let image = wallpaper_image(image, style, screen_size)?;
    let file = portal_png(&image)?;
    pollster::block_on(async {
        ashpd::desktop::wallpaper::WallpaperRequest::default()
            .set_on(ashpd::desktop::wallpaper::SetOn::Background)
            .show_preview(true)
            .build_file(file.as_file())
            .await
            .map_err(|e| format!("Could not open desktop background settings: {e}"))?
            .response()
            .map_err(|e| format!("Desktop background: {e}"))
    })
}

#[cfg(not(target_os = "linux"))]
pub fn set_wallpaper(_: &RgbaImage, _: WallpaperStyle, _: (u32, u32)) -> Result<(), String> {
    Err("Desktop background integration currently requires a Linux desktop portal.".into())
}

/// Opens a new draft in the user's mail application with the PNG attached.
/// `EmailRequest::send` sends a ComposeEmail portal request, never an email.
#[cfg(target_os = "linux")]
pub fn compose_email(image: &RgbaImage) -> Result<(), String> {
    let file = portal_png(image)?;
    let attachment = std::fs::File::open(file.path()).map_err(|e| e.to_string())?;
    pollster::block_on(async {
        ashpd::desktop::email::EmailRequest::default()
            .subject("Picture from Paint 10")
            .attach(attachment.into())
            .send()
            .await
            .map_err(|e| format!("Could not open an email draft: {e}"))?
            .response()
            .map_err(|e| format!("Email draft: {e}"))
    })
}

#[cfg(not(target_os = "linux"))]
pub fn compose_email(_: &RgbaImage) -> Result<(), String> {
    Err("Email draft integration currently requires a Linux desktop portal. Save your picture and attach it in your mail application.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_names_preserve_spaces_and_device_ids() {
        let devices = parse_scanners("epson2:libusb:001:004\tEpson Perfection V39\tflatbed scanner\ninvalid line\npixma:04A91746_1\tCanon LiDE\tflatbed scanner\n");
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].id, "epson2:libusb:001:004");
        assert_eq!(devices[0].name, "Epson Perfection V39");
        assert!(is_video_node("video42"));
        assert!(!is_video_node("video1/../../etc/passwd"));
        assert!(!is_video_node("video"));
    }

    #[test]
    fn wallpaper_layouts_preserve_requested_placement() {
        let red = Rgba([255, 0, 0, 255]);
        let white = Rgba([255, 255, 255, 255]);
        let image = RgbaImage::from_pixel(2, 1, red);
        let center = wallpaper_image(&image, WallpaperStyle::Center, (6, 5)).unwrap();
        assert_eq!(*center.get_pixel(2, 2), red);
        assert_eq!(*center.get_pixel(1, 2), white);
        let fit = wallpaper_image(&image, WallpaperStyle::Fit, (6, 5)).unwrap();
        assert_eq!(*fit.get_pixel(0, 0), white);
        assert_eq!(*fit.get_pixel(0, 1), red);
        for style in [
            WallpaperStyle::Fill,
            WallpaperStyle::Stretch,
            WallpaperStyle::Tile,
        ] {
            assert!(wallpaper_image(&image, style, (6, 5))
                .unwrap()
                .pixels()
                .all(|p| *p == red));
        }
        assert!(wallpaper_image(&image, WallpaperStyle::Tile, (0, 5)).is_err());
        assert!(wallpaper_image(&image, WallpaperStyle::Fill, (16384, 16384)).is_err());
    }

    #[test]
    fn capture_decoder_accepts_png_and_rejects_invalid_or_oversized_images() {
        let image = RgbaImage::from_pixel(3, 2, Rgba([12, 34, 56, 255]));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        assert_eq!(decode_capture(bytes.get_ref()).unwrap(), image);
        assert!(decode_capture(b"not a PNG").is_err());
        assert!(check_size(16384, 16384).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn subprocess_errors_limits_and_cancellation_are_reported() {
        let canceled = AtomicBool::new(false);
        let mut success = Command::new("sh");
        success.args(["-c", "printf captured"]);
        assert_eq!(
            run_tool(&mut success, "Test", 100, Duration::from_secs(2), &canceled).unwrap(),
            b"captured"
        );
        let mut error = Command::new("sh");
        error.args(["-c", "printf 'device disconnected' >&2; exit 7"]);
        assert!(
            run_tool(&mut error, "Test", 100, Duration::from_secs(2), &canceled)
                .unwrap_err()
                .contains("device disconnected")
        );
        let mut excessive = Command::new("sh");
        excessive.args(["-c", "printf 1234567890"]);
        assert!(
            run_tool(&mut excessive, "Test", 4, Duration::from_secs(2), &canceled)
                .unwrap_err()
                .contains("size limit")
        );
        let mut slow = Command::new("sh");
        slow.args(["-c", "while :; do :; done"]);
        assert!(
            run_tool(&mut slow, "Test", 100, Duration::from_millis(60), &canceled)
                .unwrap_err()
                .contains("timed out")
        );
        let mut cancel = Command::new("sh");
        cancel.args(["-c", "while :; do :; done"]);
        assert!(run_tool(
            &mut cancel,
            "Test",
            100,
            Duration::from_secs(2),
            &AtomicBool::new(true)
        )
        .unwrap_err()
        .contains("canceled"));
    }
}
