# Paint 10 compatibility checklist

The target is Windows 10's ribbon-based Paint. A checked feature means implemented; manual verification is recorded separately in `TESTING.md`. `PARITY_AUDIT.md` records the reopened audit and remaining differences; this checklist alone does not establish completion.

- [x] Home/View ribbon, quick save/undo/redo, palette, Color 1/2, status, scrolling canvas
- [x] Customizable Quick Access Toolbar, above/below placement, persistence and direct numeric Alt shortcuts
- [x] Pencil, brush, fill, eraser, eyedropper, magnifier
- [x] All nine brush presets and textured shape outline/fill styles
- [x] All 23 Paint shapes, multi-point polygon, two-bend curve
- [x] Rectangle and free-form selection, transparent selection, move, cut/copy/paste, crop
- [x] Selection resize handles and invert selection
- [x] New/open/save/save-as, common raster formats, unsaved changes guard, file drop
- [x] Editable project format and persistent recent files/custom colors
- [x] Resize in pixels/percent, aspect ratio, skew, canvas resize handles
- [x] Rotate 90/180/270, flips, arbitrary-angle rotation
- [x] Text/image objects remain selectable and movable during editing
- [x] In-canvas rich text entry with active move/resize handles, typed font names, collection faces, point size, bold/italic/underline/strikeout, opacity
- [x] Image properties units, DPI metadata and monochrome conversion
- [x] Zoom, rulers, pixel grid, image-only view and live Thumbnail navigation
- [x] Page setup with 19 paper presets/custom dimensions, multi-page print preview, PDF export and system print dialog
- [x] Scanner/camera import, email drafts and desktop background integration where supported
- [x] Context menus, keyboard command navigation and Paint keyboard shortcuts
- [x] Crisp vector icons, brush previews, keyboard focus and accessible control labels
- [x] Repeated isolated manual passes, Rust regressions and Nix package verification

Implementation notes: native Rust eframe/egui desktop app. Standard raster saves flatten objects; `.p10` preserves them. Text spans can be formatted independently, and shapes remain adjustable until committed. Image transparency keys preserve the original pixels. History and image inputs have memory limits.

The checklist records implemented capability, not a claim of pixel-for-pixel or exhaustive behavioral equivalence. Brush textures, font rendering, native dialogs and keyboard command presentation differ from Microsoft Paint. Scanner/camera drivers, physical printing, email composition and wallpaper portals require supported host services; these external operations were not exercised during isolated desktop testing. `TESTING.md` distinguishes automated checks, manual evidence and remaining verification limits.
