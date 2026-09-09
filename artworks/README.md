# Paint 10 artwork acceptance exercises

These pictures are being made by operating Paint 10's actual ribbon, canvas,
keyboard commands, and native save dialogs on a private Xvfb desktop. Mouse
paths are hand chosen and sent through XTEST. No finished external artwork is
imported or generated as a substitute for using the application.

## Work in progress

- Self portrait: complete, exported, reopened, and text editing retested.
- Mona Lisa recreation: complete, exported, reopened, and rotated text retested.
- AirSense 10 with AirFit P30i pixel art: complete, exports checked, reopened.
- Photorealistic landscape using only shapes: paused at the user's request.
- Widescreen ant-colony simulation slide: painted, exported and reopened; PowerPoint render verified.

Each finished exercise will include an editable `.p10` project, raster exports,
the tools used, and any defects or improvements discovered.

Detailed artwork is paused while interface polish and functionality receive
short, focused manual tests. Existing paintings and export evidence are retained.

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

## Alpine landscape study

[Project](shape-landscape.p10) · [PNG](shape-landscape.png) ·
[JPEG](shape-landscape.jpg) · [WebP](shape-landscape.webp)

A 1920 × 1080 alpine lake study, made entirely with Rectangle, Oval, Polygon,
Line and Curve tools. Linear and radial gradients build the sky, water, sun,
reflections, haze and stone shading; textured shape fills and outlines supply
snow and rock detail. No brush strokes, imported pictures or generated images
were used. The current result is stylized and remains a checkpoint toward the
requested photorealistic landscape exercise.

The saved refinements include mountain ravines, exposed rock ribs, narrow snow
channels, fragmented reflections, uneven pine branches, bent grass Curves and
foreground stones with lit faces and contact shadows. Gradient Polygons shade
the slopes; overlapping translucent Ovals build cloud lobes and distant haze.
Each pass was visually inspected in the actual app. Attempts with visible seams,
an intrusive glow or an unwanted rounded stroke end were undone.

The haze work exposed an alpha-display error. Its correction was verified on
native and browser canvases and color previews; saved RGBA pixels were intact.
The final alpine project was saved in the installed Nix package. Native
**Save a copy** refreshed PNG, JPEG and WebP while retaining the `.p10`
destination. Read-only checks confirm exact project/PNG/WebP RGBA agreement
at 1920 × 1080 and matching JPEG dimensions. The JPEG was visually inspected,
and the fresh WebP reopened in Paint 10 with the same composition.

This alpine picture remains geometric in appearance. It and its refreshed
exports are preserved alongside the paused coastal study.

## Coastal landscape in progress

[Project](coastal-landscape.p10)

A second 1920 × 1080 landscape uses Rectangle gradients for the sky and sea,
translucent radial Ovals for the light, and a thin Line for the horizon. Dragging
the reflection Oval's handles extends its gradient beyond the right and bottom
canvas edges. This initial foundation was saved through the native project
dialog and visually inspected. Fifteen translucent, pencil-outlined Curve
shapes add wave detail; a pencil-filled Rectangle adds subtle water grain.
These additions are saved. The study is paused before further detailing and
raster exports while text, color editing and interface polish are addressed.

## Ant-colony simulation slide

[Editable project](ant-colony-simulation.p10) ·
[PNG](ant-colony-simulation.png) · [PowerPoint](ant-colony-simulation.pptx)

A 1920 × 1080 teaching slide with a dark horizontal gradient, gold reinforced
trail, muted exploratory branches, nest and food markers, and a hand-drawn ant.
Rectangle, Curve, Oval, Polygon and Line tools build the diagram; Pencil draws
the ant's legs and antennae. Sixteen retained DejaVu Sans text objects provide
the title, three explanatory steps, labels, legend and footer. All artwork was
made through the actual Paint interface. The project reopened with the same
composition; double-clicking its title restored the 72-point bold DejaVu Sans
editor.

Reopening this artwork and saving it with the version 2 project format reduced
the project from 39,341,235 bytes to 1,992,405 bytes. The repeated captions now
share their embedded fonts. Reopening restores the same editable title, and
the fresh PNG export is byte-identical to the original; all project pixels,
objects, transforms, styles, font faces and resolution are unchanged.

Save a copy exported an opaque PNG while retaining the `.p10` destination.
Inspection caught an unpainted bottom row and right column, which were corrected
with Rectangle tools and re-exported. The PowerPoint contains one 16:9 slide
with that exact PNG and source notes. Reimporting and rendering it produces
identical RGBA pixels to the PNG. The diagram illustrates local rules; it does
not claim to show a measured or numerical simulation result.

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
  encoder. The encoder now writes straight-alpha metadata. Mona Lisa was
  re-exported through the real Save a copy dialog; its alpha tag is correct,
  pixels match the PNG, and ImageMagick/libtiff reports no warnings. Separate
  fixtures verify all 256 alpha values and noninteger DPI through that reader.
- Slide color setup found a stale numeric draft when reopening Edit Colors for
  white Color 2: Red displayed 0 while the actual channel was 255. The modal now
  resets draft fields before focusing; actual native replay shows synchronized
  RGB/HSL/Hex fields after reopening and preserves the project on Cancel.
