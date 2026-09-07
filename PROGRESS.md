# Paint 10 implementation

Objective: recreate Windows 10 MS Paint as a native Rust desktop app, with regular manual computer-use testing.

- [x] Raster document, drawing tools, undo/redo
- [x] Windows 10 title bar, Home/View ribbon, palette, canvas, status bar
- [x] First desktop test: drawing, shapes, colors, undo/redo
- [x] Selection, clipboard, text, file operations, image transforms
- [x] Second isolated desktop test: editing, clipboard and native save
- [x] Rebuilt app: project reopen, shape drafts, keyboard, icons, native format selector
- [ ] Final regression checks, Nix run path, documentation

Use `nix develop -c cargo run`. Native eframe/egui rendering; no web frontend.

The detailed compatibility checklist is in FEATURES.md. Manual evidence and bugs are in TESTING.md.

Current test environment: private Xvfb :1, GALLIUM_DRIVER=softpipe, keyboard/mouse via `PAINT10_TEST_DISPLAY=:1 bash tmp/gui.sh`. Launcher session 91539, logs `/tmp/paint10-desktop.2PrQyJ`. Do not use the host desktop until the restriction below expires. The delivered scripts/headless-desktop.sh launcher has run the real app with Openbox, private runtime/config directories and a private D-Bus session.

Current work: final package entrypoint verification and documentation. The source passed 71 tests, strict Clippy and Nix release tests. Manual checks verified native saving (including real 4-bit BMP), project reopen, rich text/image paste, selection resizing, adjustable shapes, two-bend curves, fullscreen, zoom/grid/rulers and small-window layout. The final modal focus bug was reproduced with an actual egui Window, fixed and manually retested: Ctrl+W → type 600 → Enter resizes correctly. All subagents have handed back their files.

Git: accidental `git add .` was cleared from the index, preserving every working file. Commits: c97ae61 ignore rules; 1b6aa0b engine; d950a65 native editor/icons; d7781d1 Nix/package/test desktop. Documentation follows. Root owns staging and logical commits. Never stage target/, tmp/, .direnv/, or result links.

## Desktop restriction — latest user steering

User requested two hours without desktop use. Timer started 2026-09-07 03:07:54 UTC; desktop use allowed again only after **2026-09-07 05:07:54 UTC** (01:07:54 America/New_York). Timer exec session: 23907.

An early GTK dialog escaped Xvfb because GTK automatically connected to the user's Wayland display despite WAYLAND_DISPLAY being unset. That process was stopped. Further tests force `GDK_BACKEND=x11`, a private runtime directory, the launcher's Xvfb DISPLAY and `dbus-run-session`; do not connect host desktop DBus/portals. No host screenshots/input during restriction.

User explicitly authorized subagents and requested readable whitespace and better code structure. Completed ownership:

- desktop_integration: app/dialogs.rs keyboard defaults/errors and HLS colors. Native Save As/raster I/O/packaging and ribbon/chrome accessibility are complete; root owns completed files.
- printing: src/icons.rs and src/icons/*, per user's explicit iconography request. Crisp Paint-like vectors, brush icons, keyboard focus/accessibility and offscreen visual contact sheet.
- document_audit: app/selection.rs and app/gestures.rs fixes for stale selection masks, text resize scale/validation and consistent pencil/eraser sizes. Engine files are stable.
- Root: app.rs/keyboard.rs/shortcuts.rs/text_editing.rs, GUI testing, integration, Git commits and final audit.

Keyboard work is integrated: global commands survive button focus and inline editing; Delete is selection-only; F11 sends real fullscreen viewport commands; Alt/F10 command navigation, Shift+F10 context menu and tab/pane cycling are available. Alt+H and fullscreen were manually verified. The vendor patch also enables Ctrl+Insert / Shift+Insert / Shift+Delete on Linux.

Do not edit an agent's files without coordinating. The vendored egui-winit patch emits an empty Paste request for image-only clipboards; upstream 0.31 consumed the shortcut. Keyup workaround removed. Upstream licenses and patch rationale included.
