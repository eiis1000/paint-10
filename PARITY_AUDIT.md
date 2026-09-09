# Paint 10 completion audit

The original requirement is a Windows 10 Paint replacement with no missing functionality. The earlier completion claim was premature: feature presence and 76 passing tests did not establish that ordinary editing workflows worked correctly. The concrete failures below have now been corrected. Complete Windows equivalence remains unverified, and the remaining differences are explicit below.

## Approved extension: optional Layers, September 9

Layers are a user-approved extension to the Windows 10 workflow. The pane is
hidden until View → Layers is enabled; ordinary pictures retain one Background
layer. The active-layer status appears when a document has multiple layers.
The stack and metadata survive `.p10` saves; normal image exports combine its
visible layers. No automatic layer is added for each stroke or object.
Native and browser verification is recorded at the top of `TESTING.md`.

## Period-video review, September 7

The current visual pass uses actual decoded menu interactions from [Hintling's
2016 Windows 10 Paint tutorial](https://www.youtube.com/watch?v=ObYvtATkrJM) and
[TutorTube's April 2021 Line Size lesson](https://www.youtube.com/watch?v=oOsQuk8Muzg).
Exact frames and timestamped sequences are recorded in
`tmp/paint-reference-video/audit.md`. These revealed a compact four-column brush
gallery at19:30, sectioned Select options at06:08, Clipboard in the contextual
Text ribbon at24:30, live Fill hover preview at34:01, and a two-column File menu
with a Save as format pane at42:04–42:07. The 2021 lesson also demonstrates live
line-width hover preview at00:33.

Those differences are implemented in `1f16473` and `e0e9c59`: the four-column
brush gallery, sectioned Select options, contextual Text Clipboard, two-column
File menu, and reversible Fill/Outline/Size hover previews. The combined native
suite passes 264 tests through `f0c68b4`; actual native File hover and browser
text/font/clipboard replay pass. The final shape-menu interaction pass continues
alongside the remaining artwork exercises. The four Size rows are visually
established; exact numeric
presets are inferred from their pixel widths, not quoted from either video.
UK “Felt tip” and “Colour” in the 2016 recording are localization differences.
No Windows 11 design changes have been adopted on the basis of this audit.

The 2016 recording's Edit Colours dialog at 22:07 also establishes 48 basic
colors in an 8×6 grid and 16 custom colors in an 8×2 grid. The old editor used
the smaller ribbon palette and only ten custom entries. Commit `6baab78`
restores these dialog counts and paired RGB/HSL columns, while adding optional
color coordinate modes, alpha and gamut fitting. Exact palette coordinates use
Wine's ChooseColor implementation as a separate reference; compressed video
pixels are not treated as exact RGB data. The inspected video shows only the
first custom insertion, so selecting an explicit overwrite slot and advancing
down columns is a chosen behavior, not a claim of proven Windows equivalence.

Actual native testing confirms named-color entry, transparent custom storage,
the separate Home recent list, invalid-input rejection and a scrollable dialog
at 500×400. The compact layout exposed a shared RGB/HSL Grid ID that requested
repaint indefinitely; distinct IDs restore the existing idle-frame regression.
The combined native suite passes 350 tests. Further browser and package results
are recorded in `TESTING.md` as those replays finish.

The follow-up `e5248af` joins the main and dropdown regions of Paste, Select
and Brushes, and gives File's format rows distinct original picture previews.
Actual native clicks open the sectioned Select menu and four-column brush
gallery; choosing Watercolor, switching to Select, and clicking the main
Brushes region restores Watercolor. Commit `20d12cc` also preserves numbers
typed immediately after F10, H, W, C. The same 80 ms keytip sequence followed
by 137 now produces a 137px custom width in the running app.

The shared source passes 316 tests (125 library + 191 app), formatting and
strict Clippy. Actual active-shape rotation keeps the canvas at 900×600,
rotates only the 283×213 rectangle to 213×283, and supports Cancel and two-step
Undo back to the clean blank picture. Unicode, browser clipboard and shared
font/project verification are recorded in `TESTING.md`.

## Corrected findings

The later September 9 quality pass fixes visibly blocky Watercolor/Oil shape
fills and overlapping translucent polygon spans. The menu pass covers all
Home galleries/dropdowns, View/Text/Image ribbons, Quick Access, context menus,
File panes and ordinary dialogs at normal and compact widths. Concrete fixes
include keytips over menu labels, the Size tile's dead area, inconsistent
control borders, clipped dialog actions, unstable Print sizing, shifting image
previews, and right-clicking Text creating a new box. Full source and manual
coverage, with hardware exclusions, is recorded in `TESTING.md`.

Image editing now retains source pixels through resizing, arbitrary rotation,
source cropping and light/color/detail adjustments. The added Image ribbon
provides grouped commands and original recovery; project version 3 retains
edits. These are deliberate extensions to Windows 10 Paint. Text and browser
limitations below still apply; this pass does not establish pixel-for-pixel
equivalence to Microsoft's brush engine.

The September 9 text/color pass retains the period Text ribbon's Clipboard,
Font, Background and Colors groups, with Paragraph, Effects and Editing as
extensions. Point sizes now use the selected face's em metrics and new boxes
use bundled regular DejaVu Sans. Existing projects retain their original
text-size convention. Space and Slice now determine the actual color plane,
including linear RGB, CMYK and perceptual gamut displays. Focused real-app
replays and exact verification limits are recorded at the top of `TESTING.md`.
Detailed artwork is paused at the user's request.

| Workflow | Concrete failure | Status |
| --- | --- | --- |
| Successive text boxes | Finishing a caption silently switches Text to Select | Actual-frame tests and native/browser successive-caption replay pass |
| Retained text double-click | A transient selection's stale handles intercept the second click | Actual-frame regression and native retained-caption reopen/edit pass |
| Text Undo ordering | Undo overtakes a final typed character and leaves it behind | Four ordered-input regressions and actual native rich-caption Undo/Redo pass |
| Browser text history | The hidden HTML input's native Undo/Redo reinserts trailing characters | Shortcut-capture regressions and actual installed-browser Undo/Redo pass |
| Color coordinate plane | Space only changes numeric fields while the visual picker stays independent | Coordinate-plane and actual-widget tests pass; native/browser space and browser CMYK Slice/Black/gamut replay pass |
| Clipboard replacement | Copying text after an image can paste the stale cached image | Reproduced before the fix; live replay now passes |
| Transparent selection copy | Copying keyed pixels loses the background permanently; switching pasted selection to Opaque cannot restore it | Copy/paste/opacity regressions pass |
| Invert colors | A transparent selection retains its old key after colors invert, reversing visible coverage | Coverage and undo regression passes |
| Text palette | Home palette changes do not format selected text; Text lacks the Paint palette | Actual-widget and live selected-word tests pass |
| Text Cut/Copy/Paste | Home clipboard commands commit/copy the whole object instead of selected characters | Widget tests pass; live Home Cut and toolbar Undo pass |
| Font size | Typing a multi-digit size can move focus into the text content | Separate-frame regression and live 46-point entry pass |
| Finish text | Ctrl+Enter is consumed before the commit handler | Regression and live replay pass |
| Active text box | Move/resize handles are absent while typing | Geometry undo regression and live reflow/move/reopen pass |
| Rotated text resize | Merely touching a handle can reflow text; vertical stretch is ignored | Transform/persistence regressions and live no-op/stretch/save/reopen pass |
| Off-canvas resize | Clipped bounds are used to resize the full source, causing position/size jumps | Regression passes; live Resize shows full 230×205 despite visible 153×205 |
| Thick shape copy | Selection bounds omit part of the stroke, including most of a thick horizontal line | All-shape tests pass; live packaged copy/paste retains the full 8px line |
| Curve/polygon preview | Passive hover changes curve bends; first polygon drag shows an unrelated polygon | Actual-frame drawing regressions pass |
| Stationary brushes | Non-airbrush strokes stamp on repaint, making darkness depend on frame count | Stationary-frame and segmentation-opacity regressions pass |
| Stroke release | Hover motion after release can extend an already completed Pencil/Eraser stroke | Separate-frame and coalesced-event pixel/Undo regressions pass |
| Cloud callout | Its tail crosses the body with an unintended diagonal line | Intersection regression and regenerated raster inspection pass |
| Keyboard navigation | Canvas focus blocks arrows; contextual tabs/keytips/text context menus are incomplete | Widget regressions pass; live text context menu verified |
| Quick Access Toolbar | Customizing commands and placing the toolbar below the ribbon are absent | Live command customization, placement and restart persistence pass |
| Thumbnail view | No thumbnail navigator exists | Reference verified; real-canvas tests and live active-text navigation pass |
| Canceled capture | A success already queued at cancellation can still insert an image | Queued-result cancellation regression passes |
| Resize dialog | A partly off-canvas object is initialized from its clipped bounds; text is rasterized | Full-object editable resize and undo regression passes |
| Skew background | Exposed wedges use white regardless of Color 2 | Fixed; focused regression passed |
| Magnifier | Zoom can move the clicked detail outside the viewport | Measured-viewport centering implemented; focused regressions passed |
| Shape gallery | Ordering and visible rows differ from the familiar Paint gallery | Seven-column gallery rendered; 500px keyboard/accessibility regression passes |
| Font families | Font names cannot be typed; collection faces other than the first are omitted | Typed-name live check and real TTC face-1 render/project regression pass |
| Unavailable paste | Failed image paste finishes an unrelated active text edit | Editor-preservation regression passes |
| Direct toolbar shortcuts | Alt+number only works after key navigation has opened | Regression passes; packaged app opens Open directly with customized Alt+4 |
| Off-canvas brushes | Pointer movement outside the image draws an unwanted edge stroke | Actual-frame regression and live out-and-back stroke pass |
| Responsive ribbon | Narrow windows require horizontal scrolling through fixed groups | Groups collapse into real command popups; final 500px package replay confirms anchored keytips and the fully visible Edit colors label |
| Anchored keytips | Alt/F10 opens a separate command window | Actual widget overlays and group/menu navigation replace it; real-frame regressions pass |
| Numeric entry | Batched typing loses a font size, and Escape can commit an unwanted draft | Both failures reproduced and corrected in actual widget tests |
| Caption preview | Large outlined text changes appearance when committed | Shared raster and character geometry; exact preview/commit test and GUI replay pass |
| Alpha compositing | Antialiased text speckles; transparent objects acquire a white background | Integer alpha and shared overlay regressions pass; rendered fixture inspected |
| Transparent Fill and eraser | Clearing retained object pixels with alpha can leave the original object visible underneath | Changed pixels are merged into the raster; tests verify one Undo restores editable objects, exact pixels and saved state |
| Canvas growth | Newly exposed pixels can hide off-canvas objects or lose alpha | Color 2 grows beneath retained objects; RGBA and gesture undo tests pass |
| Curve and line geometry | Curves include excess control-hull space; horizontal lines cannot resize vertically | Analytic cubic extrema and endpoint handles; geometry/gesture tests pass |
| Pixel workflow | Zoom and raster interpolation limit precise pixel work | 3200% zoom, 1px Pencil and independent sizes implemented; GUI nearest-neighbor export verified as exact 2× replication with all three RGBA colors preserved |
| Export workflow | Exporting copies changes the working destination | App tests preserve destination and saved revision; GUI PNG copy matches project RGBA and WebP selection matches the exact 18×18 crop |
| Native Command shortcuts | Ctrl-only matching rejects macOS Command; native close shortcuts conflict with Paint Resize | Input-frame tests cover Command/Ctrl actions; Cmd+W/Q use the unsaved close guard while physical Ctrl+W opens Resize |

Source commit `32ff8ec` passes **197 tests (84 library and 113 application)** in both the native suite and the named Nix release package. Both explicit vendored clipboard tests also pass. Native build, strict Clippy, formatting and x86_64-linux flake check pass. Verified package: `/nix/store/jrp82pfckf8kdqnw0y8klnfb2nzs648b-paint-10-0.1.0`; release log: `tmp/nix-package-verified.log`. Detailed manual evidence is in `TESTING.md`; assertions that check egui widgets use real input frames, including dialog/overlay ordering where relevant.

The ordinary GUI pixel exercise saved an exact RGBA PNG copy of a 32×24 project, scaled it to 64×48 with exact nearest-neighbor replication, and exported an exact 18×18 WebP selection. `tmp/check_pixel_exports.py` verifies the saved artifacts. The reopened scaled PNG visibly retains transparency in `/tmp/paint10-transparent-reopened-settled.png`. The five final artwork acceptance exercises have begun as a separate pass and are not covered by these ordinary verification results.

Additional improvement: flipping an object now preserves editable text through its transform. Its raster comparison and undo regression pass.

Independent detailed source reports remain under `tmp/selection-parity-audit.md`, `tmp/ui-parity-audit.md`, and `tmp/drawing-parity-audit.md`. These scratch reports are excluded from Git; this document and `TESTING.md` preserve the conclusions.

## Remaining differences and verification limits

- Home, View, and contextual Text now adapt into compressed groups at narrow widths. Alt/F10 labels are anchored to the real controls and menus, with group Tab traversal, arrow navigation, nested Escape handling, and restoration of text/canvas focus. Actual-frame regressions cover 500px and 1200px layouts, including resizing open menus. Exact Paint key letters and pixel placement remain independently mapped; these follow Microsoft's [ribbon interaction guidance](https://learn.microsoft.com/en-us/windows/win32/uxguide/cmd-ribbons), rather than an exhaustive comparison with a running Windows 10 installation.
- Brushes and textured fills are independent raster implementations. Controlled Windows 10 samples have not established matching stroke dynamics or pixel output.
- Reopening transformed text uses an upright editor, then restores its transform on completion. Active vector-line resizing preserves stroke thickness.
- Native windows have a 500×400 minimum, which the color-dialog layout checks cover. An exploratory 360px browser viewport clips the dialog's left edge; layouts below that native minimum need a separate browser pass.
- Physical printer/scanner/camera and host email/wallpaper portal operations have not been exercised. The ARM Linux package output evaluates, but was not built or executed on this host.
- Windows/macOS native builds and native dialogs have not been run locally. The checked-in CI matrix is not evidence of execution; the native Command checks above simulate the relevant input modifiers on Linux. Linux provides the hardware/desktop integrations; Windows/macOS Print currently saves a printable PDF.

No exhaustive comparison against a running Windows 10 Paint installation has been performed. These checks support the listed fixes, not an unconditional claim of complete functional or visual parity.
