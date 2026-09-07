# Paint 10 clipboard patch

This directory contains egui-winit 0.31.1 from crates.io, with three related changes:

- Every paste shortcut emits `Event::Paste`, even when the clipboard has no text.
- Ctrl/Cmd+Shift/Alt+C/X/V combinations reach the application instead of being
  consumed as ordinary clipboard commands. Paint uses Ctrl+Shift+X for Crop and
  Ctrl+Shift+V for Paste from a file.
- Ctrl+Insert, Shift+Insert, and Shift+Delete work on Linux as well as Windows,
  preserving Paint's alternative copy, paste, and cut shortcuts in text and images.

Upstream 0.31.1 consumes Ctrl+V before the application receives the keydown. It
only emits Paste for nonempty text, so an image-only clipboard cannot reliably
be pasted by a drawing application. Key-release fallbacks depend on modifier
release order and do not solve the problem.

Paint 10 handles the Paste request by reading the image clipboard. Empty Paste
events do not insert characters into text widgets. Dedicated clipboard keys and
Insert/Delete alternatives are preserved across platforms. Remove this patch when
upgrading to a backend that exposes image paste requests and application-specific
modified clipboard shortcuts directly.
