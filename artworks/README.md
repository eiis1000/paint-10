# Paint 10 artwork acceptance exercises

These pictures are being made by operating Paint 10's actual ribbon, canvas,
keyboard commands, and native save dialogs on a private Xvfb desktop. Mouse
paths are hand chosen and sent through XTEST. No finished external artwork is
imported or generated as a substitute for using the application.

## Work in progress

- Self portrait: complete, exported, reopened, and text editing retested.
- Mona Lisa recreation: complete, exported, reopened, and rotated text retested.
- AirSense 10 with AirFit P30i pixel art: complete, exports checked, reopened.
- Photorealistic landscape using only shapes: underway.
- Widescreen ant-colony simulation slide: pending.

Each finished exercise will include an editable `.p10` project, raster exports,
the tools used, and any defects or improvements discovered.

## Self portrait

[Editable project](self-portrait.p10) · [PNG](self-portrait.png) · [JPEG](self-portrait.jpg)

A stylized robotic self portrait on a 720 × 560 canvas. Painted with Brush,
Calligraphy, Airbrush, Oil, Crayon, Marker, Natural pencil, and Watercolor,
plus a filled oval and rounded rectangle foundation. The title and small
caption remain separate editable text objects. Native Save As wrote the
project; Save a copy exported PNG and JPEG while retaining the project
destination. Both exports were opened and visually inspected.
The project reopened with matching appearance. Double-clicking CODEX with
Select restored its original 30-point bold format in the contextual Text tab.

## Mona Lisa

[Editable project](mona-lisa.p10) · [PNG](mona-lisa.png) · [TIFF](mona-lisa.tiff)

A geometric, painterly poster reinterpretation on a 420 × 560 canvas. Polygon
hills and clothing, ellipse face forms, watercolor atmosphere, oil shading,
pencil and textured brush details, and curve-tool fabric folds surround the
crossed hands and small smile. The serif title is retained text rotated 90°
left; two small captions are also editable. Save a copy exported PNG and TIFF
without changing the project destination. The PNG was visually inspected.
Reopening the project preserved the composition and rotation. Double-clicking
the title with Select restored its original DejaVu Serif, 16-point bold editor.

## AirSense 10 and AirFit P30i

[Editable project](airsense-10-p30i.p10) · [PNG](airsense-10-p30i.png) ·
[GIF](airsense-10-p30i.gif) · [8× PNG](airsense-10-p30i-8x.png)

A 96 × 64 pixel drawing with 13 opaque colors, made at 800% zoom with the pixel
grid. Filled rectangles, polygons, and ellipses establish the machine, mask,
and hose; the Pencil adds individual highlights, display symbols, hose ribs,
and hand-drawn lettering. The original project, PNG, and GIF have identical
RGBA pixels. Resize with **Keep hard pixel edges** produced the enlarged PNG;
every source pixel is replicated into an exact 8 × 8 block. Save a copy kept
the project destination, and Undo restored its original dimensions afterward.
The enlarged export was visually inspected, and the original project reopened
correctly in the verified Nix package.

## Findings

- Canvas setup: entering 720 and 560 in Image Properties yielded 7 × 16384.
  The rebuilt GUI now produces 720 × 560 at the original 45 ms typing speed,
  after preserving focus-event order and selecting the existing number on Tab.
- A completed oval followed immediately by a Brush ribbon click became a
  partial oval and stray brush stroke. The event-order regression and rebuilt
  GUI replay now retain the full oval without a stray stroke.
- Enter now applies an adjustable shape. Its regression verifies subsequent
  palette changes leave the applied shape intact.
- Follow-up double-click testing found that queued clicks lost their timing
  on a slow frame. The queue now retains arrival timestamps; tests include
  both 30 ms and 700 ms frame intervals. Native replay also passes.
- Pixel canvas setup found early width typing discarded while Properties was
  acquiring initial focus. The modal queue now retains that input. The same
  Ctrl+E, immediate 96, Tab, 64 sequence yields 96 × 64 in the packaged GUI.
- Independent TIFF inspection found a missing alpha-channel tag in the pinned
  encoder. A correction and GUI re-export are in progress; the current opaque
  Mona Lisa TIFF pixels match its PNG, but its metadata is not yet accepted.
