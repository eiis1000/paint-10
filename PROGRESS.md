# Paint 10 implementation

Objective: recreate Windows 10 MS Paint as a native Rust desktop app, with regular manual computer-use testing.

**Reopened audit, 2026-09-07:** the previous completion claim was premature. The concrete workflow fixes listed in PARITY_AUDIT.md are implemented and verified. All three agents have handed back their files. Root integrated clipboard behavior, editable transforms, toolbar preferences/direct shortcuts, and repeated real GUI tests. The original requirement of complete Windows Paint equivalence has not been established; remaining differences are explicit in the audit.

Current verification: 122 release tests (62 library, 60 application), native build, strict Clippy, formatting and x86_64-linux flake check pass. Package `/nix/store/pc6vxlmfsvn05v0j0fwqbj4midwbbywd-paint-10-0.1.0` was launched and manually exercised. Current manual evidence is at the top of TESTING.md. Scratch projects/screenshots remain outside Git.

Both reopened-audit GUI sessions closed cleanly. The final packaged session was `/tmp/paint10-desktop.z1i8U7`; its temporary stroke and paste were undone before closing. The saved regression project remains `/tmp/paint10-audit-transforms.p10`. No host desktop input was used for these checks.

Current implementation commits: acca55d (toolbar settings/vector command icons), b423e4e (editing, transforms, drawing, fonts, ribbon, keyboard and Thumbnail). No source changes are pending. The following notes describe the earlier, insufficient verification pass and must not override the reopened audit.

## Earlier implementation pass

- [x] Raster document, drawing tools, undo/redo
- [x] Windows 10 title bar, Home/View ribbon, palette, canvas, status bar
- [x] First desktop test: drawing, shapes, colors, undo/redo
- [x] Selection, clipboard, text, file operations, image transforms
- [x] Second isolated desktop test: editing, clipboard and native save
- [x] Rebuilt app: project reopen, shape drafts, keyboard, icons, native format selector
- [x] Final regression checks, Nix run path, documentation

Use `nix develop -c cargo run`. Native eframe/egui rendering; no web frontend.

The detailed compatibility checklist is in FEATURES.md. Manual evidence and bugs are in TESTING.md.

The final private test session has been closed cleanly. Logs are in `/tmp/paint10-desktop.r0PVOy`; the preceding packaged `nix run` session used `/tmp/paint10-desktop.Za3puC`. The delivered scripts/headless-desktop.sh launcher ran the real app with Openbox, private runtime/config directories and a private D-Bus session. Do not use the host desktop until the restriction below expires.

Implementation and final verification are complete for this pass. The source passed 76 tests, strict Clippy, formatting and Nix release tests; `nix flake check .` passed on x86_64-linux. The final code package is `/nix/store/72lmhr841l444q3w6bi7mbib095zn3jz-paint-10-0.1.0`. Manual checks verified native saving (including real 4-bit BMP), project reopen, rich text/image paste, selection resizing, adjustable shapes, two-bend curves, polygon completion, arbitrary rotation, fullscreen, zoom/grid/rulers and small-window layout. Custom-paper validation, native PDF export and print preview were verified together. All subagents have handed back their files.

Remaining verification limits: no exhaustive comparison against a Windows installation, no physical printer/scanner/camera test, no host wallpaper/email portal invocation, and no aarch64 build. See FEATURES.md and TESTING.md for precise coverage; do not claim complete behavioral equivalence.

Git: accidental `git add .` was cleared from the index, preserving every working file. Logical commits cover ignore rules, the engine, the native editor/icons, Nix packaging/test desktop, and documentation. Final fixes are 60cf07d (standard/custom paper sizes) and fa5673e (Clear Picture with a selection). Never stage target/, tmp/, .direnv/, or result links.

## Desktop restriction — latest user steering

User requested two hours without desktop use. Timer started 2026-09-07 03:07:54 UTC; desktop use allowed again only after **2026-09-07 05:07:54 UTC** (01:07:54 America/New_York). Timer exec session: 23907.

An early GTK dialog escaped Xvfb because GTK automatically connected to the user's Wayland display despite WAYLAND_DISPLAY being unset. That process was stopped. Further tests force `GDK_BACKEND=x11`, a private runtime directory, the launcher's Xvfb DISPLAY and `dbus-run-session`; do not connect host desktop DBus/portals. No host screenshots/input during restriction.

User explicitly authorized subagents and requested readable whitespace and better code structure. Completed ownership:

- desktop_integration: native Save As, raster I/O, packaging, ribbon/chrome accessibility, modal keyboard behavior and HLS colors. Complete.
- printing: iconography requested by the user, PDF generation/preview, and standard/custom paper sizes. Complete; actual vector artwork and rendered PDFs inspected.
- document_audit: engine audit, selection masks, text resizing, project/DPI handling and package verification. Complete.
- Root: integration, editable text/shape workflows, keyboard behavior, repeated GUI testing, Git commits and final audit. Complete for this pass.

Keyboard work is integrated: global commands survive button focus and inline editing; Delete is selection-only; F11 sends real fullscreen viewport commands; Alt/F10 command navigation, Shift+F10 context menu and tab/pane cycling are available. Alt+H and fullscreen were manually verified. The vendor patch also enables Ctrl+Insert / Shift+Insert / Shift+Delete on Linux.

Do not edit an agent's files without coordinating. The vendored egui-winit patch emits an empty Paste request for image-only clipboards; upstream 0.31 consumed the shortcut. Keyup workaround removed. Upstream licenses and patch rationale included.
