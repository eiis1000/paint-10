# Paint 10

Paint 10 is a native Rust desktop drawing application built around the Windows 10 Paint ribbon and workflow. It targets Linux, Windows, and macOS using egui/eframe. Linux supports Wayland and X11 and uses GTK file dialogs.

## Run

With Nix and flakes enabled, run from this directory:

```sh
nix run path:.
```

To open a picture at startup:

```sh
nix run path:. -- /path/to/picture.png
```

The explicit `path:.` also works when the directory is not a Git checkout. `flake.lock` pins the development and packaging dependencies. The package includes the scanner and camera helper programs and installs a desktop launcher and icon. Building does not install or activate the launcher in your desktop profile.

```sh
nix build path:.
./result/bin/paint-10
```

The flake provides packages and development shells for `x86_64-linux` and `aarch64-linux`. A graphical desktop session and working OpenGL driver are needed to run the application. Printing, wallpaper settings, and email drafts use your desktop's XDG portals; availability depends on the portal backend and installed applications.

To include it in a NixOS configuration, add this checkout or its Git location
as a flake input named `paint10`, pass your inputs to the module, and use:

```nix
{ inputs, pkgs, ... }:
{
  environment.systemPackages = [
    inputs.paint10.packages.${pkgs.stdenv.hostPlatform.system}.paint-10
  ];
}
```

Home Manager can put the same package in `home.packages`. The flake also
exports `overlays.default` if you prefer to install it as `pkgs.paint-10`.
Adding the package installs the application and launcher; it does not change
desktop settings or configure scanners, cameras, or portal services.

## Draw and edit

Use the Home ribbon for brushes, shapes, fill, eraser, text, selections, colors, and image transformations. Color 1 is the foreground; Color 2 is the background. Right-click a palette swatch to choose Color 2. The View ribbon controls zoom, rulers, gridlines, and picture view. Above 100% zoom, enable Thumbnail to navigate the picture through a floating preview.

Shapes remain adjustable until applied; press Enter to apply a finished shape.
The Fill menu also offers vertical, horizontal, and radial gradients from
Color 1 to Color 2. A transparent Color 2 gives a soft edge for light and mist.
These optional fills use V, H, and R keytips after opening Fill; the original
Paint fill choices retain their numeric keytips.

The title-bar dropdown customizes the Quick Access Toolbar and moves it below the ribbon. Its commands and placement persist. Alt+1, Alt+2, and subsequent numbers invoke the commands in their current order.

At narrow widths, ribbon groups collapse into buttons that open their full controls. Alt or F10 displays keytips on the actual commands; type the displayed letters to activate them. Tab moves between groups, arrows navigate within a group, and Escape returns through open menus.

Text and inserted images remain editable objects. Select an object to move or resize it; double-click a text object to edit its contents. While typing, drag the box border to move it or its handles to reflow the text. The text ribbon offers typed/searchable installed fonts, size, styles, the Paint palette, and an opaque or transparent background. Home clipboard commands act on selected characters while a text box is active. Rotation includes arbitrary angles as well as the familiar quarter turns and flips; object resizing and flips preserve editable text.

For meme captions, the Text ribbon includes left/center/right alignment and adjustable text outlines. The editable preview uses the same text layout and pixels as the saved picture. A white bold caption with a black outline stays legible over a photograph.

For pixel art, use the 1-pixel Pencil, enable Gridlines, and zoom up to 3200%. Pencil, brush, eraser, and shape widths are remembered separately. Resize's **Keep hard pixel edges (pixel art)** option uses nearest-neighbor scaling. **Transparent Color 2** lets you clear, erase, fill, or grow a transparent canvas; right-click with Fill to use Color 2. The checkerboard shows empty pixels. Fill and eraser operations that remove opacity from existing objects can merge their visible pixels into the raster; one Undo restores the pixels and editable objects.

For precise distances, enable **View → Measure distance** and drag between two
pixel centers. Drag either endpoint to adjust it, or use arrow keys to move the
active endpoint by one pixel (Shift: ten). The readout shows distance, horizontal
and vertical displacement, and angle. Millimeters, centimeters and inches use
the picture's horizontal and vertical DPI. Delete resets the measurement;
Escape leaves Measure. The ruler is an overlay and does not alter saved pixels
or undo history.

Save as a **Paint 10 project (`.p10`)** to retain editable text, image objects, and their transforms. Saving PNG, JPEG, BMP, GIF, or TIFF exports the visible raster picture; reopening those formats gives a flattened image. Undo history is kept for the current session and is not stored in project files. Destructive raster operations, including lifting a raster selection or transforming the whole canvas, can merge editable objects; undo can restore the previous state while it remains in history.

Projects embed the fonts used by their captions, so those captions remain
editable on another computer or in the browser. Version 2 projects store each
font once, even when many captions use it; older version 1 projects still open.
Text editing supports mixed writing directions, shaped scripts, Unicode word
selection, and deletion of complete characters including combining marks.

The Save As format selector updates the filename extension and supports monochrome, 16-color, 256-color, and 24-bit BMP files, as well as PNG, JPEG, GIF, TIFF, WebP, icons, and editable projects. JPEG aliases such as `.jpe` and bitmap files named `.dib` are accepted. Indexed BMP export reduces the picture to the selected number of colors.

Page Setup offers Letter, Legal, Tabloid, Executive, Statement, A0–A6, ISO B4/B5, photo and envelope presets, plus custom dimensions in millimeters. Orientation, individual margins, centering, actual-size scaling, and fitting across multiple pages apply to both print preview and PDF export. Actual size uses the image's DPI. Invalid dimensions or margins disable printing and export until corrected.

The canvas is limited to 16 megapixels and 16,384 pixels on either axis. Undo history has a memory budget. Editable projects support up to 1,000 objects and 128 MB of object data; invalid or oversized saves preserve the existing destination. Large scanned pictures may need a lower capture resolution.

## Keyboard shortcuts

| Action | Shortcut |
| --- | --- |
| New / Open / Save | Ctrl+N / Ctrl+O / Ctrl+S |
| Save as | F12 or Ctrl+Shift+S |
| Undo / Redo | Ctrl+Z / Ctrl+Y |
| Cut / Copy / Paste | Ctrl+X / Ctrl+C / Ctrl+V |
| Alternative Cut / Copy / Paste | Shift+Delete / Ctrl+Insert / Shift+Insert |
| Paste from a file | Ctrl+Shift+V |
| Select all | Ctrl+A |
| Resize and skew | Ctrl+W |
| Image properties | Ctrl+E |
| Crop selection | Ctrl+Shift+X |
| Invert colors / Clear picture | Ctrl+Shift+I / Ctrl+Shift+N |
| Print | Ctrl+P |
| Gridlines / Rulers | Ctrl+G / Ctrl+R |
| Zoom | Ctrl+mouse wheel |
| Zoom in / out | Ctrl+PageUp / Ctrl+PageDown |
| Increase / decrease tool size | Ctrl+Plus / Ctrl+Minus |
| Cancel or deselect | Escape |
| Picture view | F11 |
| Ribbon command navigation | Alt or F10; Alt+F / H / V |
| Selection context menu | Shift+F10 |
| Cycle Home / View / contextual Text | Ctrl+Tab / Ctrl+Shift+Tab |
| Focus canvas / ribbon | F6 |
| Collapse the ribbon | Ctrl+F1 |

In a text box, Ctrl+B, Ctrl+I and Ctrl+U format the selection; Ctrl+Z/Ctrl+Y undo and redo text edits. Ctrl+Enter commits the box. Home, View and Text remain available while editing. Dialogs support Enter to accept and Escape to cancel.

On macOS, Command works for the corresponding document and text shortcuts, including clipboard, formatting, undo/redo, and zoom; physical Ctrl remains available. Cmd+W and Cmd+Q close through the unsaved-change prompt. Use physical Ctrl+W for Paint's Resize and skew command.

## Devices and desktop integration

Scanner import uses SANE's `scanimage`; camera capture uses FFmpeg's V4L2 input. The Nix environment supplies both. Device drivers, scanner configuration, and access permissions belong to your host system. Device discovery does not take a picture; capturing requires choosing a device and starting the operation.

Wallpaper actions open the desktop portal with a preview. Email integration opens a draft with a PNG attachment for you to address and send. Paint 10 does not send mail itself. These integrations require supporting desktop services; hardware and portal behavior cannot be guaranteed solely by the Rust build.

## Development and verification

```sh
nix develop path:. -c cargo run
nix develop path:. -c cargo test
nix develop path:. -c cargo fmt --check
nix develop path:. -c cargo clippy --all-targets
nix flake check path:.
```

The development shell supplies Rust, native libraries, GTK schemas, and the capture helpers without modifying your desktop settings. The packaged executable carries its runtime environment in a wrapper.

The [verification log](TESTING.md) records source-specific native and browser
checks, Nix package builds, and actual isolated mouse/keyboard workflows.
Windows/macOS native execution remains unverified locally. Artwork acceptance
and any remaining implementation work are tracked separately in
[PROGRESS.md](PROGRESS.md).

For manual testing on an isolated desktop:

```sh
nix develop path:.#test -c scripts/headless-desktop.sh
```

This starts Paint 10 on a private Xvfb display with its own D-Bus session, GTK settings, and temporary user directories. The script prints the display number and log directory. The test shell includes mouse/keyboard control and screenshot tools. To run an existing binary instead, pass its command after the script name.

[FEATURES.md](FEATURES.md) tracks implementation coverage; [TESTING.md](TESTING.md) records actual automated and manual verification. [PARITY_AUDIT.md](PARITY_AUDIT.md) records corrected failures and remaining differences, including brush rendering and untested hardware integration. The aim is familiar Paint behavior with useful editing improvements. This project is an independent implementation and does not claim complete behavioral or visual equivalence with Microsoft Paint.

The ordinary GUI pixel exercise verified an exact RGBA PNG copy, an exact 2× nearest-neighbor resize preserving all three colors and transparency, and an exact 18×18 WebP selection export. Reopening the PNG retained transparency. The final 500px package replay also verified the palette keytips and wrapped Edit colors label. The five artwork acceptance exercises form a separate verification pass.

## Source layout

`src/lib.rs` exposes the document model, raster tools, text rendering, project format, image codecs, metadata and printing. The native binary and browser library compile the same `src/app/` modules for gestures, selections, text editing, ribbon controls, dialogs, keyboard commands and file operations. `src/icons/` contains vector artwork without a dependency on symbol fonts. The small vendored egui-winit patch preserves image paste and Paint's modified clipboard shortcuts; its rationale and upstream licenses are included alongside the source.

## Browser

The same Rust application also compiles to WebAssembly and runs locally in a
desktop browser with WebGL. Pictures are processed on your device; the browser
target is a static site and does not upload images to a server.

```sh
nix develop path:.#web -c bash scripts/build-web.sh
nix develop path:.#web -c python3 -m http.server 8080 --bind 127.0.0.1 --directory target/web
```

Open `http://127.0.0.1:8080/`. `nix build .#paint-10-web` produces the packaged
static site. The [browser guide](web/README.md) also covers building without Nix.

Open imports pictures and editable `.p10` projects. Save, Save as, Save a copy,
and Save selection download files in the shared raster/project formats;
Download PDF uses the shared page layout. Font list → Load font imports local
TTF, OTF, and TTC files, and used fonts are embedded in saved projects. Opening
a project makes its embedded font families available in the font list. Browser
preferences preserve custom colors and Quick Access settings.

Browsers cannot confirm that a download dialog finished, so downloading keeps
the document marked as modified. Before New/Open, verify the downloaded file
and then choose **Don't save** in the pending prompt. Reloading or closing a
modified picture triggers the browser's unsaved-work warning. Clipboard access
depends on browser permissions; standard paste events and explicit Paste from
remain available. Native desktop integrations require the desktop application.

## Windows, macOS, and portable exports

The Rust application targets Linux, Windows, and macOS. Linux has been built and exercised locally; the Windows and macOS native builds are covered by the checked-in [build workflow](.github/workflows/build.yml), which must run on those systems before their results can be claimed. With a Rust toolchain and the platform's C/C++ build tools installed:

```sh
cargo build --locked --release
cargo test --locked --all-targets
```

The executable is `target/release/paint-10` on Linux/macOS and `target/release/paint-10.exe` on Windows. Windows requires the Visual Studio C++ build tools; macOS requires Xcode Command Line Tools. On Linux outside Nix, install GTK 3 development files, `pkg-config`, and X11/Wayland/OpenGL development libraries. The workflow lists the Ubuntu packages used for its Linux build.

Linux Save As uses GTK's native format selector. Windows and macOS first show all eleven formats, including each BMP color depth, then open the native destination dialog. This keeps BMP depth explicit even though the variants share `.bmp`. Preferences use `%APPDATA%` on Windows, `~/Library/Application Support` on macOS, and `~/.config` on Linux; an absolute `XDG_CONFIG_HOME` overrides those locations.

File → **Save a copy…** writes another file while preserving the current filename and saved revision. **Save selection as…** exports only the selected pixels or object. Both offer the existing raster and project formats. PNG, TIFF, WebP, and icons preserve RGBA pixels; GIF supports a transparent palette entry; JPEG and BMP composite transparency over white. Opening transparent images retains their alpha. Saving a `.p10` copy retains the document's editable objects.

Scanner/camera capture, wallpaper, and email attachment integration currently use Linux services. On Windows/macOS, those hardware/desktop integrations are unavailable; Print offers a printable PDF to save and print using a PDF application. Drawing, editing, image/project files, page layout, and PDF generation use the shared Rust implementation.

## Use from a NixOS flake

The flake exports `packages.<system>.paint-10` and an identical `default`, for `x86_64-linux` and `aarch64-linux`. Add this checkout or its repository as a flake input, then include the package in a NixOS module:

```nix
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.paint10.url = "path:/path/to/paint";
  # For a remote checkout, replace the path input with its actual Git URL.

  outputs = inputs@{ nixpkgs, ... }: {
    nixosConfigurations.my-machine = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ./configuration.nix
        ({ pkgs, ... }: {
          environment.systemPackages = [
            inputs.paint10.packages.${pkgs.stdenv.hostPlatform.system}.paint-10
          ];
        })
      ];
    };
  };
}
```

Alternatively, add `inputs.paint10.overlays.default` to `nixpkgs.overlays` and install `pkgs.paint-10`. The overlay builds against the consuming package set. The package includes runtime wrappers, a desktop entry, and the application icon; installing it does not activate any scanner, camera, or desktop operation. Use `nix build .#paint-10` to build the named package locally.
