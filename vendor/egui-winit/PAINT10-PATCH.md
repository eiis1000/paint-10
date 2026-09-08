# Paint 10 keyboard, clipboard and IME patch

This directory contains egui-winit 0.31.1 from crates.io, with six related changes:

- Every paste shortcut emits `Event::Paste`, even when the clipboard has no text.
- Ctrl/Cmd+Shift/Alt+C/X/V combinations reach the application instead of being
  consumed as ordinary clipboard commands. Paint uses Ctrl+Shift+X for Crop and
  Ctrl+Shift+V for Paste from a file.
- Ctrl+Insert, Shift+Insert, and Shift+Delete work on Linux as well as Windows,
  preserving Paint's alternative copy, paste, and cut shortcuts in text and images.
- Clipboard shortcuts accept both native Command and physical Ctrl on macOS.
  egui reports Command separately from Ctrl there; on Linux, its `command` flag
  follows Ctrl and is never set by Super. Shift/Alt variants still reach Paint.
- Physical Ctrl+A retains Paint's select-all behavior in macOS text fields.
  Its key event also sets egui's logical Command flag, avoiding the default
  Control+A start-of-line action. Other Control navigation keeps its native behavior.
- Native IME candidate placement uses `IMEOutput.cursor_rect`, scaled to physical
  pixels, instead of the entire text field's `rect`. A Paint text box can cover
  most of the canvas, and its caret can move across multiple lines. The existing
  cached-area comparison now updates when that caret moves. This follows egui's
  explicit primary-cursor rectangle and winit's candidate-area contract; on X11
  the backend uses only the area's position. Actual native candidate windows
  still require platform-specific input-method verification.

Upstream 0.31.1 consumes Ctrl+V before the application receives the keydown. It
only emits Paste for nonempty text, so an image-only clipboard cannot reliably
be pasted by a drawing application. Key-release fallbacks depend on modifier
release order and do not solve the problem.

Paint 10 handles the Paste request by reading the image clipboard. Empty Paste
events do not insert characters into text widgets. Dedicated clipboard keys and
Insert/Delete alternatives are preserved across platforms. Remove this patch when
upgrading to a backend that exposes image paste requests and application-specific
modified clipboard shortcuts directly.
