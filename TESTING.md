# Paint 10 verification log

## Environment

- Native Rust eframe/egui 0.31.1, Nix dev shell, Rust 1.97.1.
- The shared KDE Wayland desktop locked during implementation. Computer Use screenshots confirmed the lock screen. No session settings were changed.
- Xvfb display `:99` (1280×900) is used for manual mouse/keyboard tests. Screenshots use the screenshot skill helper. Actions use XTEST/xdotool against the actual application window.
- Isolation correction: GTK initially auto-discovered the host Wayland socket and opened Save on the user's desktop. The test process was stopped. The launcher now forces `GDK_BACKEND=x11`, creates a private runtime directory, and uses `dbus-run-session`. A new screenshot of Xvfb itself confirms the native Save dialog is contained there: `/tmp/paint10-truly-isolated-save.png`.
- The user's desktop must not be accessed until 2026-09-07 05:07:54 UTC, per their two-hour timer request.
- Mesa 26.1.8 llvmpipe stalled in `lp_fence_wait` on the isolated display. GDB stacks localized it to the software rasterizer. The isolated test uses `GALLIUM_DRIVER=softpipe`; the application does not force a graphics driver or disable vsync.

## First manual pass

- Initial ribbon, palette, canvas, title and status render. Screenshot: `/tmp/paint10-softpipe.png`.
- Rectangle outline and fill work; fill remains inside the border. Screenshot: `/tmp/paint10-fill.png`.
- Ctrl+Z removes a committed text insertion, Ctrl+Y restores it. Screenshot: `/tmp/paint10-undo-focused.png`.
- In-canvas text entry and contextual font ribbon work. Screenshot: `/tmp/paint10-text-focused.png`.
- Fixed missing dropdown glyphs.
- Fixed fast drags beginning at the latest cursor position. Retested stroke and rectangle start/end coordinates: `/tmp/paint10-drag-retest.png`.
- Fixed double-clicking unselected text when clicks coalesce into one frame. Reopening text verified: `/tmp/paint10-double-fixed.png`.

## File dialog pass

- Found a GTK crash when saving because GSettings schemas were absent. Fixed the Nix environment and package wrapper.
- Saved `/tmp/paint10-native-save.png` through the native dialog with keyboard input; returned to the canvas with the updated title and saved status: `/tmp/paint10-save-complete.png`.

## Rich text and clipboard pass

- Typed `Hello world`, selected only `world` with Ctrl+Shift+Left, and applied Ctrl+B/Ctrl+I. Only the selected word changed: `/tmp/paint10-word-styles.png`.
- Ctrl+Z removed italic while retaining bold and the text: `/tmp/paint10-style-undo.png`. Ctrl+Y restored italic.
- Committing text retained both styles in the raster rendering. Copied the selected object and pasted it with Ctrl+V; a second visible image appeared at the canvas origin: `/tmp/paint10-image-paste-fixed.png`.
- This exposed an egui-winit 0.31 input limitation: it consumed image-only paste shortcuts without emitting a Paste event. A minimal vendored patch now preserves the request; the unreliable key-release fallback was removed.
- The actual page preview rendered the test oval within the configured page: `/tmp/paint10-print-preview.png`.
- Closing the scratch document displayed Save / Don't save / Cancel: `/tmp/paint10-close-prompt.png`.

## Automated checks

- Initial 7 tests passed: fill boundaries, history/saved state, canceled previews, clipped paste, rotation/canvas sizing, text/PNG round-trip, PDF layout.
- The integrated suite passed 71 tests (49 library and 22 UI tests), including image metadata, bitmap bit depths, rich text/history, transformed selections, project bounds and modal keyboard behavior. Strict `cargo clippy --all-targets -- -D warnings` and formatting checks passed. The three dialog keyboard tests now use an actual modal Window, and passed again after the manual focus fix.
- Six focused printing tests pass. Letter, A4 landscape with asymmetric margins, and all four tiled PDF pages were rendered with Poppler and visually inspected. Fixtures are in `tmp/pdfs/`.

## Manual coverage

The passes below cover drawing and fill, rectangle/ellipse/curve, rich text, selected-object movement/resizing, clipboard, native save/reopen, print preview, fullscreen, keyboard navigation, modal entry, rulers/grid/zoom and small-window layout. Polygon completion, arbitrary-angle rotation and every brush/selection combination are implemented and covered in part by core tests, but have not all received an exhaustive manual workflow pass.

## Rebuilt app and icon pass

- The delivered `scripts/headless-desktop.sh` was used to launch the real app on dynamically allocated Xvfb display `:1`, with Openbox, private runtime/config/data directories and a private D-Bus session. This verifies the advertised test-shell entrypoint.
- Reopened `/tmp/paint10-richtext.p10`; both the original styled text and the independently resized pasted image render correctly: `/tmp/paint10-new-ui.png`. The older binary had displayed a stale preview; the project data and current library rendering were intact.
- Alt+H, H, 4 selected Rectangle using keyboard commands: `/tmp/paint10-keytips.png`.
- A released rectangle retained its draft handles. Moving, resizing, outline recoloring and textured fill all updated the same shape: `/tmp/paint10-shape-draft.png`, `/tmp/paint10-shape-moved-color.png`, `/tmp/paint10-shape-resized.png`, `/tmp/paint10-shape-textured.png`. One Ctrl+Z removed the entire adjusted shape; Ctrl+Y restored it.
- Dragging both curve bends produced an adjustable S-shaped curve: `/tmp/paint10-curve-first.png`, `/tmp/paint10-curve-complete.png`.
- F11 occupied the full isolated screen and Escape restored the editor: `/tmp/paint10-fullscreen.png`.
- Ctrl+Shift+S opened the new native Save As helper. Its dropdown exposed all formats and bitmap depths. Selecting 16-color BMP produced `/tmp/paint10-export-16.bmp`: Windows 3.x BMP, 900×600, **4 bits per pixel**, with 96 DPI metadata. Screenshots: `/tmp/paint10-format-options.png`, `/tmp/paint10-bmp-name.png`, `/tmp/paint10-bmp-saved.png`.
- The icon subagent rendered and inspected the actual vector artwork at ribbon sizes, all 23 shapes, nine brushes/stroke samples, and selected/disabled/focused button states. Contact sheet: `tmp/icons-contact-sheet.png`. AccessKit checks verified names and states. Final ribbon inspection: `/tmp/paint10-final-ui.png`.
- Manual Resize testing caught initial focus being consumed by egui's invisible window-layout pass. The test was changed from a generic panel to an actual modal Window, which reproduced the failure. After the fix, Ctrl+W selected the width; typing `600` and pressing Enter resized 900×600 to 600×400; Ctrl+Z restored the original image. Screenshots: `/tmp/paint10-resize-focus-fixed.png`, `/tmp/paint10-resize-enter-fixed.png`.
- Ctrl+R and Ctrl+G toggle rulers and gridlines. Ctrl+PageUp zooms the picture; the grid is visible at 400%: `/tmp/paint10-grid-400.png`. An 800×600 window retains the title controls, scrolling ribbon, canvas scrollbars and zoom controls: `/tmp/paint10-small-window.png`.
- `nix build . --no-link` and `nix flake check .` passed for x86_64-linux. The package's release suite passed all 71 tests. The package wrapper includes GTK schemas, graphics libraries and capture helpers. Aarch64 was not built on this host.

## Verification limits

The manual passes exercise the actual Rust application with mouse and keyboard input; they are not exhaustive equivalence tests against a Windows installation. Physical printers/scanners/cameras and host wallpaper/email portals were not invoked. The host desktop has remained untouched since isolation was corrected. Screenshots and scratch exports are temporary evidence, deliberately excluded from Git.
