# Paint 10 completion audit

The original requirement is a Windows 10 Paint replacement with no missing functionality. The earlier completion claim was premature: feature presence and 76 passing tests did not establish that ordinary editing workflows worked correctly. The concrete failures below have now been corrected. Complete Windows equivalence remains unverified, and the remaining differences are explicit below.

## Corrected findings

| Workflow | Concrete failure | Status |
| --- | --- | --- |
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

The integrated source passes 122 tests (62 library and 60 application), including in the release package. Native build, strict Clippy, formatting and x86_64-linux flake check pass. The final Nix package runs the saved project through its actual wrapped executable. Detailed manual evidence is in `TESTING.md`; assertions that check egui widgets use real input frames, including dialog/overlay ordering where relevant.

Additional improvement: flipping an object now preserves editable text through its transform. Its raster comparison and undo regression pass.

Independent detailed source reports remain under `tmp/selection-parity-audit.md`, `tmp/ui-parity-audit.md`, and `tmp/drawing-parity-audit.md`. These scratch reports are excluded from Git; this document and `TESTING.md` preserve the conclusions.

## Remaining differences and verification limits

- Narrow windows scroll the ribbon horizontally instead of collapsing native ribbon groups. Commands remain reachable; the interaction and presentation differ.
- Alt/F10 command navigation uses a separate command window instead of Microsoft's keytip overlays and group focus model.
- Brushes and textured fills are independent raster implementations. Controlled Windows 10 samples have not established matching stroke dynamics or pixel output.
- Reopening transformed text uses an upright editor, then restores its transform on completion. Curve bounds can contain excess control-hull space, and active vector-line resizing preserves stroke thickness.
- Physical printer/scanner/camera and host email/wallpaper portal operations have not been exercised. Aarch64 packaging was not built on this host.

No exhaustive comparison against a running Windows 10 Paint installation has been performed. These checks support the listed fixes, not an unconditional claim of complete functional or visual parity.
