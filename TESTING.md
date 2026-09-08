# Paint 10 verification log

## September 8: shared native and browser verification

The source checkpoint through browser commit `f0c68b4` passes **264 native
tests (97 library + 167 application)**, formatting, strict Clippy and the native
debug build. The named Nix release package also passes all 264 tests:
`/nix/store/c59jksasbd08gh7nir2d81i6d2vwr4xh-paint-10-0.1.0`. Logs are
`tmp/native-menu-clean-gate.log` and `tmp/nix-native-final-committed.log`.
`nix flake check .` passes on x86_64-linux
(`tmp/nix-flake-final-committed.log`); incompatible aarch64 outputs were omitted
from that host check.

The browser target compiles the same Rust application. Strict WASM Clippy and
the release build pass in the pinned `.#web` shell, with Rust 1.97.1 and
wasm-bindgen 0.2.127. Logs: `tmp/wasm-final-clippy.log` and
`tmp/browser-final-release.log`. Its Nix static package also builds successfully:
`/nix/store/33n46jimrdnk4l9d95pa101slsvpqqqr-paint-10-web-0.1.0`. The packaged
HTML matches source, every generated JavaScript import exists, and the WASM
header is valid (`tmp/nix-browser-final.log`,
`tmp/nix-browser-final-static-check.log`). This package includes the final
browser production fixes; native-only debug tracing was removed afterward and
does not affect the WASM runtime.

The application regressions cover consecutive and batched File arrows, entering
the PNG format row, menu-to-editor shortcut focus, reversible shape-style
previews, exact custom brush sizes, compact Edit Colors geometry and no-op
rotated text edits. The **23 focused text tests**, included in the 264-test
total, also verify Unicode Clipboard/history at wide and 500px widths, compact
font galleries, scrolling to the last family, and registering project fonts
without changing encoded project bytes or pixels
(`tmp/font-gallery-project-fonts.log`).

Actual browser tests used private Chromium on Xvfb with a private profile,
loading the release site at `http://127.0.0.1:8080/`. Mouse/keyboard actions and
download dialogs ran inside that private desktop. The final screenshots and
rendered PDF below were visually inspected.

- **Theme and rulers:** a late browser system-theme event originally replaced
  Paint's square menu styling with egui defaults. The refreshed build retains
  the intended styling; the actual-button regression passes late Light/Dark/Light
  events. Ctrl+R now toggles rulers without also reloading the page
  (`/tmp/paint10-browser-ctrl-r-fixed.png`). F5 remains available for reload.
- **Downloads and discard protection:** an actual pencil stroke was downloaded
  as PNG. Canceling the first destination dialog retained the modified state
  and the browser's reload warning
  (`/tmp/paint10-browser-canceled-download-guard.png`). Subsequent Ctrl+S correctly
  opened the destination dialog directly after the filename was retained.
  Successful exports `/tmp/paint10-browser-first-download.png` and
  `/tmp/browser-pencil-september8.png` were visually inspected and byte-identical.
  File → New after opening a `.p10` project reset Save to PNG
  (`/tmp/paint10-browser-menu-new-png.png`).
- **Fonts and retained text:** importing `DejaVuSans.ttf` succeeded. Reopening
  the Mona Lisa project exposed its embedded DejaVu Serif family in the compact
  font gallery, with the final Load font row visible and no large blank area
  below it (`/tmp/paint10-browser-final-font-gallery.png`). Escape followed by
  Ctrl+Enter closed the gallery and committed the rotated title without
  reopening the menu or marking the unchanged project dirty
  (`/tmp/paint10-browser-final-font-commit.png`).
- **Clipboard permissions and keyboard paste:** blocking the ribbon Paste
  permission request left the document unchanged and displayed the denial
  explanation. Paste options → Paste copied selection explicitly inserted the
  locally copied image (`/tmp/paint10-browser-paste-local-fallback.png`). The
  initial silent Ctrl+V failure came from eframe stopping paste propagation at
  its document text listener. With image capture handled before that listener,
  actual Ctrl+C/Ctrl+V pasted the selected title even while Clipboard API access
  remained blocked (`/tmp/paint10-browser-final-capture-paste.png`). One Undo
  removed the pasted image and returned the project to its clean state
  (`/tmp/paint10-browser-final-paste-undo.png`).
- **Browser PDF:** Page setup, landscape orientation and print preview rendered
  the actual shape landscape. Download PDF produced
  `/tmp/paint10-browser-landscape.pdf`: one landscape Letter page, **792 × 612
  points**. Its rendered page `/tmp/paint10-browser-landscape-page.png` was
  inspected for content, orientation and placement. This verifies the export
  workflow; it does not certify the landscape's separate artwork acceptance
  requirement.

These results remain bounded. Browser downloads cannot confirm completion of
the destination dialog, so they retain the unsaved-work guard. Chrome reserves
Ctrl+N for a browser window and Ctrl+Page Up/Down for tab navigation; use File →
New and the View/status zoom controls for the picture. The
[browser guide](web/README.md) records these limits and the local font/file and
clipboard workflows. This pass does not claim manual browser coverage of every
export format or every drawing-tool combination.

Windows/MSVC and Apple/Darwin dependency trees resolve with `--locked --offline`
(`tmp/native-dependencies-windows-resumed.txt`,
`tmp/native-dependencies-macos-resumed.txt`), but neither target was compiled or
executed on this Linux host. The native CI matrix is configured; no external CI
run is claimed. Physical hardware integrations remain untested. Artwork
acceptance is tracked separately in `artworks/README.md` and `PROGRESS.md`.
After this checkpoint, manual Edit Colors reopening exposed a stale Red numeric
field when switching from an edited Color 1 to white Color 2. Commit `ce9fa9f`
clears the prior numeric draft before initial modal focus is assigned. The
regression reproduced displayed Red `0` alongside numeric value 255, then
verified all reopened RGB/HSL fields, Hex, subsequent editing and Cancel.
All 11 dialog and 12 Properties tests, strict Clippy and the refreshed native
build pass (`tmp/color-reopen-fixed-gate.log`). Actual native replay now shows
Red/Green/Blue 255 and Hex FFFFFF after editing Color 1, with the project clean
(`/tmp/paint10-ant-fixed-color-reopened.png`).

The refreshed native Nix package after `ce9fa9f` is
`/nix/store/64a87iz71vf7wr3xxsqmipxrsywv7mxg-paint-10-0.1.0`; all **265 release
tests (97 library + 168 app)** and flake checks pass. The matching refreshed web
package is `/nix/store/jsgw1a4xm6sqv6ym33x36i0yy729jk01-paint-10-web-0.1.0`;
WASM strict Clippy, release and static checks pass.

Additional actual browser checks:

- At a narrow window, Home/View groups and their menus fit. Measuring A(9,9)
  to B(12,13) gives 5.00px and 53.13 degrees. Right then Shift+Right moves B to
  (23,13): 14.56px, 15.95 degrees and 3.852mm at 96dpi. All readout fields fit,
  and the project stays clean. Captures use
  `/tmp/paint10-browser-final-measure-five500.png` and `-measure-mm500.png`.
- The Mona Lisa project was opened and actually downloaded again as `.p10`.
  Read-only comparison with `tmp/check-browser-project.py` verifies identical
  image bytes, objects, transforms, fonts and other project values. The newer
  serializer explicitly writes three empty `font_faces` defaults omitted by the
  older source project, so the compressed files are not byte-identical.
  Download: `/tmp/paint10-browser-mona-roundtrip.p10`.

Earlier results below retain their original source snapshots and scope.

## Responsive ribbon, captions, transparency and portability pass

Ordinary verification for source commit `32ff8ec` passes **197 tests (84 library + 113 application)** in both the native suite and the named Nix release package. Strict Clippy, formatting, the native build and x86_64-linux flake check pass. The two vendored clipboard tests also pass when run explicitly; CI now includes that command. Windows/macOS native execution remains unverified locally. The five final artwork acceptance exercises have begun as a separate pass and are not counted in these results.

Verified release package: `/nix/store/jrp82pfckf8kdqnw0y8klnfb2nzs648b-paint-10-0.1.0`. Build log: `tmp/nix-package-verified.log`. The live application sources matched the package's source snapshot. The ARM Linux package output evaluates, but was not built or executed on this host.

- **Large outlined captions:** the first live 54-point caption had mismatched foreground/outline glyphs (`/tmp/paint10-caption-54-live.png`). The editor and raster renderer used different font-size units. Live glyph pixels and caret geometry now come from the document renderer. Actual typing, white/bold/centered/outlined formatting, word replacement and Ctrl+Enter produce matching live and committed text (`/tmp/paint10-caption-live-shared-fixed.png`, `/tmp/paint10-caption-edited-shared.png`, `/tmp/paint10-caption-committed-shared.png`). App-frame regressions compare the texture in the typing frame with the committed raster and click within centered glyphs at 100%/400% zoom.
- **500px ribbon:** the native minimum width now exposes collapsed groups, all tool commands and a readable zoom/status area (`/tmp/paint10-ribbon-real-500.png`). Keyboard ZK opens the palette with its anchored keytips (`/tmp/paint10-500-color-keytips.png`). Switching to Shapes closes Colors, leaving a single popup (`/tmp/paint10-500-sibling-groups.png`). This exposed clipped Edit colors text; the final `jrp…` package replay confirms anchored keytips (`/tmp/paint10-final-500-colors.png`) and the fully wrapped, unobstructed label (`/tmp/paint10-final-500-colors-label.png`). Both final screenshots were visually inspected.
- **Keyboard event failures:** real-frame tests reproduced selected text being replaced by a synthetic Enter, stale popup ancestry after resize, mouse-menu Arrow/Escape failures, fast multi-digit font input and numeric Escape committing an unwanted draft. The controller tests now pass all those cases using actual widgets.
- **Native Command shortcuts:** input-frame tests cover macOS Command and physical Ctrl for document commands, text formatting/history, Select all and wheel zoom. Cmd+W/Cmd+Q close through the unsaved-change guard; physical Ctrl+W retains Paint's Resize and skew command. The explicit egui-winit tests cover clipboard recognition and physical Ctrl normalization. These are shared input tests on Linux, not a claim of a native macOS run.
- **Transparency and geometry:** shared alpha compositing avoids dark speckles and opacity loss. Library/gesture tests cover transparent editable objects, transparent Fill and eraser undo, exact RGBA preservation, Color 2 beneath newly exposed off-canvas objects, line endpoints and tight cubic bounds. Fill with transparent or translucent Color 2 merges visible objects into the filled raster when pixels change; one Undo restores the pixels, editable objects and saved state. No-op fills preserve the objects. The caption fixture remains byte-identical after adding shared editing geometry.
- **Stroke release boundary:** a real-frame regression reproduced Pencil/Eraser extending a finished click when the same frame also contained later hover motion. Gesture endpoints and collected stroke motion now stop at release. Separate-frame and coalesced press/release cases pass, including pixel and Undo checks.
- **Saving and portability:** codec tests cover exact RGBA and indexed palettes. App tests cover Save a copy/selection without changing the current destination, including an object wholly outside the canvas. All eleven choices and Cancel are exercised in the portable format picker. A real subprocess regression verifies Save As can launch after the running development executable has been replaced. Both NixOS consumption examples evaluate to the same package; the native CI matrix is checked in but has not run externally.
- **Pixel-art exports through the real GUI:** saved the 32×24 project `/tmp/paint10-pixel-probe.p10` and PNG copy `/tmp/paint10-pixel-probe-copy.png`; every RGBA byte matches. Resizing with nearest-neighbor interpolation produced `/tmp/paint10-pixel-scaled.png` at 64×48, with every source pixel replicated into an exact 2×2 block. All three original RGBA colors remain, with alpha values only 0 and 255. Save selection as produced `/tmp/paint10-pixel-selection.webp`, an exact 18×18 RGBA crop of the selected region. The read-only `tmp/check_pixel_exports.py` checks these assertions and passed again while documenting this evidence. Reopening the scaled PNG retains its checkerboard transparency (`/tmp/paint10-transparent-reopened-settled.png`, visually inspected).

## Earlier reopened audit after the completion challenge

The earlier 76-test pass was insufficient. `PARITY_AUDIT.md` records the concrete failures found afterward and the remaining differences. The following checks were performed against rebuilt native executables, using real mouse and keyboard input on private Xvfb desktops.

- **Clipboard replacement and text completion:** reproduced the old stale-image paste after copying characters (`/tmp/paint10-stale-clipboard-confirmed.png`). The rebuilt app finishes text with Ctrl+Enter and reports no picture on a text-only clipboard, preserving the drawing (`/tmp/paint10-audit-clipboard-fixed.png`). An unsuccessful image paste also keeps active text open in a focused regression.
- **Text palette and clipboard:** only `second` changed color in `First second` (`/tmp/paint10-audit-word-blue-settled.png`). Home Copy/Cut removed only the selected characters; Quick Access Undo restored the word and its color (`/tmp/paint10-audit-home-cut.png`, `/tmp/paint10-audit-text-qat-undo.png`).
- **Font entry:** typed `4` and `6` in separate rendered frames, then Enter; the font became 46 points without inserting digits into the text (`/tmp/paint10-audit-font-46.png`). Typed `DejaVu Serif` into the new font-name field for one selected word. Its changed rendering is visible in `/tmp/paint10-audit-thumbnail-navigated-text.png`. The automated font test constructs a real two-face TTC, selects face 1, compares its raster against the standalone font and round-trips the face through `.p10`.
- **Active text handles:** resized the typing box until two words reflowed onto separate lines, moved its border, committed and reopened it at the new position (`/tmp/paint10-audit-text-handle-wrap.png`, `/tmp/paint10-audit-text-handle-move.png`, `/tmp/paint10-audit-moved-text-reopened-settled.png`). Actual-frame tests also check geometry undo/redo.
- **Transformed text:** a 90° text rotation produced 130×205 bounds. Touching its handle preserved both pixels and the saved title; a horizontal drag stretched it to 230×205 without reflow (`/tmp/paint10-audit-noop-stays-saved.png`, `/tmp/paint10-audit-rotated-text-stretched.png`). Saved through the native `.p10` format selector and reopened with the transform intact (`/tmp/paint10-audit-project-reopened.png`).
- **Off-canvas resize:** moved that object partly outside the canvas. The status reported visible bounds of 153×205 while Resize correctly initialized the full 230×205 dimensions (`/tmp/paint10-audit-offcanvas-resize-dialog.png`).
- **Stroke clipping:** an out-and-back brush drag left two clipped diagonal segments and no unwanted horizontal stroke along the canvas edge (`/tmp/paint10-audit-outside-brush.png`). In the packaged app, copying and pasting an active 8-pixel horizontal line retained all eight rows, with 208×8 pasted bounds (`/tmp/paint10-audit-thick-line-paste.png`).
- **Quick Access Toolbar:** added Open, moved the toolbar below the ribbon, inspected its private preferences file, closed and reopened the app, and verified both settings persisted (`/tmp/paint10-audit-qat-persisted.png`). Manual testing exposed missing direct Alt-number handling; after fixing it, customized Alt+4 opened the native Open dialog in the final package (`/tmp/paint10-audit-packaged-alt4.png`).
- **Thumbnail and keyboard context:** at 200%, enabled Thumbnail while editing text, then clicked its preview to scroll the active text box into view without committing it (`/tmp/paint10-audit-thumbnail-navigated-text.png`). Shift+F10 displayed text-specific commands (`/tmp/paint10-audit-text-context-menu.png`). The Thumbnail regression was verified to fail without the overlay-layer guard and pass with it using the actual canvas → thumbnail → dialogs update order.
- **Drawing and gallery regressions:** actual-frame checks cover stationary marker opacity, brush continuation across event batches, curve stability between bend drags, the polygon's first-edge preview and no-op selection handles. Raster checks cover all 23 shape bounds at widths 1/3/8/50, noncrossing Cloud callout geometry and rounded callout corners. The regenerated raster/contact sheets were visually inspected. The 500px ribbon regression checks keyboard scrolling, activation and accessible names in both gallery presentations.

Historical verification for that earlier audit: **122 tests passed (62 library, 60 application)** in its Nix release package. Native build, strict `cargo clippy --all-targets -- -D warnings`, formatting and `nix flake check .` passed on x86_64-linux. Historical release log: `tmp/final-package-build.log`. Package tested then: `/nix/store/pc6vxlmfsvn05v0j0fwqbj4midwbbywd-paint-10-0.1.0`.

The live development session used `/tmp/paint10-desktop.kWQhCQ`; the final packaged session used `/tmp/paint10-desktop.z1i8U7`. The software renderer can lag input; screenshots above were inspected after the relevant state settled. The early 45 ms-per-digit font-entry attempt on the old binary did **not** reproduce the focus bug and is not cited as failure evidence.

The earlier passes below are historical evidence, not proof that the original requirement was already complete. Hardware/portal checks and exhaustive Windows equivalence remain unverified; see `PARITY_AUDIT.md`.

## Environment

- Native Rust eframe/egui 0.31.1, Nix dev shell, Rust 1.97.1.
- The shared KDE Wayland desktop locked during implementation. Computer Use screenshots confirmed the lock screen. No session settings were changed.
- Xvfb displays (1280×900) are used for manual mouse/keyboard tests: initially `:99`, then dynamically allocated `:1` and `:2` through the delivered launcher. Screenshots use the screenshot skill helper. Actions use XTEST/xdotool against the actual application window.
- Isolation correction: GTK initially auto-discovered the host Wayland socket and opened Save on the user's desktop. The test process was stopped. The launcher now forces `GDK_BACKEND=x11`, creates a private runtime directory, and uses `dbus-run-session`. A new screenshot of Xvfb itself confirms the native Save dialog is contained there: `/tmp/paint10-truly-isolated-save.png`.
- The user's two-hour desktop restriction expired at 2026-09-07 05:07:54 UTC. The reopened audit continued using private desktops.
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
- The earlier integrated suite passed 76 tests (54 library and 22 UI tests), including image metadata, bitmap bit depths, rich text/history, transformed selections, project bounds, custom paper sizes and modal keyboard behavior. Strict `cargo clippy --all-targets -- -D warnings` and formatting checks passed. The three dialog keyboard tests use an actual modal Window, and passed again after the manual focus fix.
- Twelve printing tests pass. Letter, A4 landscape with asymmetric margins, Legal, A3 landscape, custom 100×160 mm paper, and every page of two four-page tiled PDFs were rendered with Poppler and visually inspected. Fixtures are in `tmp/pdfs/`.

## Manual coverage

The passes below cover drawing and fill, rectangle/ellipse/curve/polygon, rich text, selected-object movement/resizing/rotation, clipboard, native save/reopen, print preview and PDF export, fullscreen, keyboard navigation, modal entry, rulers/grid/zoom and small-window layout. Every brush/selection combination has not received an exhaustive manual workflow pass.

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
- At this earlier stage, `nix build . --no-link` and `nix flake check .` passed for x86_64-linux, and that package's release suite passed all 76 tests. The package wrapper includes GTK schemas, graphics libraries and capture helpers. Aarch64 was not built on this host.

## Earlier packaged workflow pass

- Launched the packaged entrypoint with `nix develop .#test -c scripts/headless-desktop.sh nix run . -- /tmp/paint10-richtext.p10`: `/tmp/paint10-nix-run.png`.
- Rotated the selected pasted image to 33° through the angle dialog. Its bounds changed from 420×70 to 391×288; the separate editable text stayed unchanged. Ctrl+Z restored the image: `/tmp/paint10-rotate-33.png`.
- Drew the polygon's first edge, added vertices, and double-clicked to complete a four-sided adjustable shape. One Ctrl+Z removed it: `/tmp/paint10-polygon-complete.png`, `/tmp/paint10-polygon-undo.png`.
- Fixed Ctrl+Shift+N clearing only the selected object. The shortcut now clears the whole picture; one Ctrl+Z restores both objects: `/tmp/paint10-clear-selection-before.png`, `/tmp/paint10-clear-picture.png`, `/tmp/paint10-clear-undo.png`.
- Opened the paper presets and custom dimensions through Page Setup. A zero width displayed a validation error and disabled Print, Save PDF and Preview. Entering 100×160 mm restored them: `/tmp/paint10-paper-presets.png`, `/tmp/paint10-custom-invalid.png`, `/tmp/paint10-custom-100x160.png`.
- Saved `/tmp/paint10-manual-custom.pdf` through the isolated native dialog. Poppler reported one page with a 283.465×453.543-point MediaBox, matching 100×160 mm. The rendered PDF and live preview show the same content placement: `/tmp/paint10-manual-custom.png`, `/tmp/paint10-custom-preview.png`.

## Verification limits

The manual passes exercise the actual Rust application with mouse and keyboard input; they are not exhaustive equivalence tests against a Windows installation. Physical printers/scanners/cameras and host wallpaper/email portals were not invoked. The host desktop has remained untouched since isolation was corrected. Screenshots and scratch exports are temporary evidence, deliberately excluded from Git.
