# Paint 10

Paint 10 is a native Rust desktop drawing application built around the Windows 10 Paint ribbon and workflow. Linux is the primary platform, with Wayland and X11 support. It uses egui/eframe for the interface and GTK for file dialogs.

## Run

With Nix and flakes enabled, run from this directory:

```sh
nix run path:.
```

To open a picture at startup:

```sh
nix run path:. -- /path/to/picture.png
```

The explicit `path:.` also works when the directory is not a Git checkout. `flake.lock` pins the development and packaging dependencies. The package includes the scanner and camera helper programs and installs a desktop launcher and icon. Building does not install or activate the launcher in your desktop profile.

```sh
nix build path:.
./result/bin/paint-10
```

The flake provides packages and development shells for `x86_64-linux` and `aarch64-linux`. A graphical desktop session and working OpenGL driver are needed to run the application. Printing, wallpaper settings, and email drafts use your desktop's XDG portals; availability depends on the portal backend and installed applications.

## Draw and edit

Use the Home ribbon for brushes, shapes, fill, eraser, text, selections, colors, and image transformations. Color 1 is the foreground; Color 2 is the background. Right-click a palette swatch to choose Color 2. The View ribbon controls zoom, rulers, gridlines, and picture view. Above 100% zoom, enable Thumbnail to navigate the picture through a floating preview.

The title-bar dropdown customizes the Quick Access Toolbar and moves it below the ribbon. Its commands and placement persist. Alt+1, Alt+2, and subsequent numbers invoke the commands in their current order.

Text and inserted images remain editable objects. Select an object to move or resize it; double-click a text object to edit its contents. While typing, drag the box border to move it or its handles to reflow the text. The text ribbon offers typed/searchable installed fonts, size, styles, the Paint palette, and an opaque or transparent background. Home clipboard commands act on selected characters while a text box is active. Rotation includes arbitrary angles as well as the familiar quarter turns and flips; object resizing and flips preserve editable text.

Save as a **Paint 10 project (`.p10`)** to retain editable text, image objects, and their transforms. Saving PNG, JPEG, BMP, GIF, or TIFF exports the visible raster picture; reopening those formats gives a flattened image. Undo history is kept for the current session and is not stored in project files. Destructive raster operations, including lifting a raster selection or transforming the whole canvas, can merge editable objects; undo can restore the previous state while it remains in history.

The Save As format selector updates the filename extension and supports monochrome, 16-color, 256-color, and 24-bit BMP files, as well as PNG, JPEG, GIF, TIFF, WebP, icons, and editable projects. JPEG aliases such as `.jpe` and bitmap files named `.dib` are accepted. Indexed BMP export reduces the picture to the selected number of colors.

Page Setup offers Letter, Legal, Tabloid, Executive, Statement, A0–A6, ISO B4/B5, photo and envelope presets, plus custom dimensions in millimeters. Orientation, individual margins, centering, actual-size scaling, and fitting across multiple pages apply to both print preview and PDF export. Actual size uses the image's DPI. Invalid dimensions or margins disable printing and export until corrected.

The canvas is limited to 16 megapixels and 16,384 pixels on either axis. Undo history has a memory budget. Editable projects support up to 1,000 objects and 128 MB of object data; invalid or oversized saves preserve the existing destination. Large scanned pictures may need a lower capture resolution.

## Keyboard shortcuts

| Action | Shortcut |
| --- | --- |
| New / Open / Save | Ctrl+N / Ctrl+O / Ctrl+S |
| Save as | F12 or Ctrl+Shift+S |
| Undo / Redo | Ctrl+Z / Ctrl+Y |
| Cut / Copy / Paste | Ctrl+X / Ctrl+C / Ctrl+V |
| Alternative Cut / Copy / Paste | Shift+Delete / Ctrl+Insert / Shift+Insert |
| Paste from a file | Ctrl+Shift+V |
| Select all | Ctrl+A |
| Resize and skew | Ctrl+W |
| Image properties | Ctrl+E |
| Crop selection | Ctrl+Shift+X |
| Invert colors / Clear picture | Ctrl+Shift+I / Ctrl+Shift+N |
| Print | Ctrl+P |
| Gridlines / Rulers | Ctrl+G / Ctrl+R |
| Zoom | Ctrl+mouse wheel |
| Zoom in / out | Ctrl+PageUp / Ctrl+PageDown |
| Increase / decrease tool size | Ctrl+Plus / Ctrl+Minus |
| Cancel or deselect | Escape |
| Picture view | F11 |
| Ribbon command navigation | Alt or F10; Alt+F / H / V |
| Selection context menu | Shift+F10 |
| Cycle Home / View / contextual Text | Ctrl+Tab / Ctrl+Shift+Tab |
| Focus canvas / ribbon | F6 |
| Collapse the ribbon | Ctrl+F1 |

In a text box, Ctrl+B, Ctrl+I and Ctrl+U format the selection; Ctrl+Z/Ctrl+Y undo and redo text edits. Ctrl+Enter commits the box. Home, View and Text remain available while editing. Dialogs support Enter to accept and Escape to cancel.

## Devices and desktop integration

Scanner import uses SANE's `scanimage`; camera capture uses FFmpeg's V4L2 input. The Nix environment supplies both. Device drivers, scanner configuration, and access permissions belong to your host system. Device discovery does not take a picture; capturing requires choosing a device and starting the operation.

Wallpaper actions open the desktop portal with a preview. Email integration opens a draft with a PNG attachment for you to address and send. Paint 10 does not send mail itself. These integrations require supporting desktop services; hardware and portal behavior cannot be guaranteed solely by the Rust build.

## Development and verification

```sh
nix develop path:. -c cargo run
nix develop path:. -c cargo test
nix develop path:. -c cargo fmt --check
nix develop path:. -c cargo clippy --all-targets
nix flake check path:.
```

The development shell supplies Rust, native libraries, GTK schemas, and the capture helpers without modifying your desktop settings. The packaged executable carries its runtime environment in a wrapper.

For manual testing on an isolated desktop:

```sh
nix develop path:.#test -c scripts/headless-desktop.sh
```

This starts Paint 10 on a private Xvfb display with its own D-Bus session, GTK settings, and temporary user directories. The script prints the display number and log directory. The test shell includes mouse/keyboard control and screenshot tools. To run an existing binary instead, pass its command after the script name.

[FEATURES.md](FEATURES.md) tracks implementation coverage; [TESTING.md](TESTING.md) records actual automated and manual verification. [PARITY_AUDIT.md](PARITY_AUDIT.md) records corrected failures and remaining differences, including narrow-window ribbon behavior, keyboard command presentation, brush rendering, and untested hardware integration. The aim is familiar Paint behavior with useful editing improvements. This project is an independent implementation and does not claim complete behavioral or visual equivalence with Microsoft Paint.

## Source layout

`src/lib.rs` exposes the document model, raster tools, text rendering, project format, image codecs, metadata and printing. The binary owns `src/app/`, whose modules separate gestures, selections, text editing, ribbon controls, dialogs, keyboard commands and file operations. `src/icons/` contains vector artwork without a dependency on symbol fonts. The small vendored egui-winit patch preserves image paste and Paint's modified clipboard shortcuts; its rationale and upstream licenses are included alongside the source.
