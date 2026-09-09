# Paint 10 verification log

## September 9: reopened measurement and Paste from findings

The read-only completion audit reproduced three defects in the current app:
New retains measurements from the previous canvas, three queued Right presses
move a measurement endpoint only one pixel, and native Paste from rejects a
valid `.p10` offered by its own file filter. The isolated probes exercise the
real app frame/input path and native decoder (`tmp/completion-probes.rs` and
`tmp/completion-probes.log`). Commits `334d476` and `cc13446` correct these
failures, with seven native file and fourteen measurement tests passing.

Actual native replay on `cc13446` opens the private Paste from chooser and
inserts `airsense-10-p30i.p10` into a blank 900×600 picture as a selected 96×64
image. Dragging moves it from (0,0) to (220,162). One Undo restores its original
position; the next removes the insertion and restores the clean Untitled
picture. The project source is unchanged. Captures were visually inspected:
`/tmp/paint10-native-project-{pasted,moved-settled,move-undo,undo-view}.png`.

In the same native app, measuring A(42,188) to B(192,388) gives 250.00px.
Three immediate Right presses produce B(195,388), Δx153 and 251.81px. New
clears the endpoints while keeping Measure enabled. Drawing another ruler and
opening the 96×64 CPAP project also clears the endpoints, leaving no off-canvas
labels. Captures were visually inspected:
`/tmp/paint10-native-measure-{fixed-baseline,three-arrows-fixed,new-fixed,open-fixed}.png`.
The old-browser reproductions below remain before-fix evidence.

The installed browser also reproduces the New defect through its actual File
menu: A(76,89), B(220,280) and the 239.20px reading survive File → New.
Both `/tmp/paint10-browser-measure-before-new.png` and
`/tmp/paint10-browser-measure-menu-new-before-fix.png` were visually inspected.
Ctrl+N opened a Chromium window, as already documented; that window was closed
and the actual Paint File menu was used for the reproduction.

In that same browser session, three immediate Right presses moved B from
(220,280) to (221,280), confirming the dropped steps in actual keyboard input.
`/tmp/paint10-browser-measure-three-arrows-before-fix.png` was visually
inspected; the expected endpoint is (223,280).

Opening the 96×64 CPAP project then leaves both old labels outside the small
canvas and retains the stale distance. The actual capture
`/tmp/paint10-browser-measure-open-before-fix.png` was visually inspected.

The refreshed `a292591` browser now moves B(220,280) to B(223,280) for the
same three immediate Right presses: Δx147 and 241.02px. File → New clears
both endpoints, and opening the 96×64 CPAP project after drawing another ruler
also clears them. Measure remains enabled and the project stays clean. These
actual screenshots were visually inspected:
`/tmp/paint10-browser-measure-{fixed-baseline,three-arrows-fixed,new-fixed,open-fixed}.png`.

## September 9: light transparent color preview defect

Painting haze exposed a separate display discrepancy. Native and browser
Edit Colors both show RGB 192/196/185, alpha 96, and exact `C0C4B960`, while
their New and Home swatches appear white instead of a muted checkerboard.
The native Current swatch also appears white when reopening the same color.
This persists after the dialog settles. Captures were visually inspected:
`/tmp/paint10-landscape-haze-color-{check,settled}.png` and
`/tmp/paint10-browser-light-alpha-preview.png`. The numeric color and saved
painting are separate from this preview failure.

The actual browser also paints the same color as a No-outline Solid Rectangle
on a canvas cleared with transparent Color 2. Its on-screen rectangle is white.
Saving through the browser's PNG download and private native destination dialog
produces `/tmp/paint10-alpha-fixture.png`. Independent ImageMagick inspection
confirms 900×600, with 27,456 pixels of exact `(192,196,185,96)` and 512,544
fully transparent pixels. The screenshot and saved data therefore demonstrate
a display-conversion failure, not lost PNG alpha. This real drawing is the
fixture for the corrected native/browser texture replay.

The pinned constructor/shader probe predicts the observed clipping: the old
display value `[123,126,118,96]` becomes almost white over both checker colors;
the needed byte-premultiplied value is `[72,74,70,96]`. Correct document
composites are `[206,208,204]` over gray 215 and `[231,233,229]` over white.
`tmp/alpha-display-probe.log` records the constructor and screenshot pixel
measurements. The correction below keeps display conversion separate from
image storage and raster compositing.

Commit `a292591` corrects Paint-owned display colors and textures without
changing document RGBA or exports. The combined source passes 365 tests
(142 library + 223 app), strict native/WASM Clippy, formatting, native debug
and browser release builds, and all eight JavaScript adapter tests. Exact
source manifests and logs are in `tmp/display-measurement-combined-native-handoff.md`
and `tmp/alpha-display-handoff.md`.

Actual native and browser Open both display the GUI-saved alpha fixture as a
muted translucent rectangle, retaining the checkerboard. Entering `C0C4B960`
in Edit Colors matches that canvas, and native acceptance/reopening retains
matching Current/New/Home swatches. All corrected screenshots were visually
inspected: `/tmp/paint10-{native,browser}-alpha-{canvas,editor}-fixed.png` and
`/tmp/paint10-native-alpha-current-settled.png`.

Independent read-only screenshot measurements compare 25,841 checker-interior
pixels with the actual document compositor. Every channel agrees within one
8-bit level, including native software-renderer dithering. Full region
histograms and geometric edge exclusions are recorded in
`tmp/alpha-{native,browser}-fixed-pixels.log`. This checks solid colors and
nearest-neighbor canvas pixels; it does not claim exact agreement at the
pinned renderer's linearly filtered subpixel preview edges.

## September 9: installed browser color replay

The immutable `6baab78` web package is served at port 8082 and was operated
in private Chromium. F10 → H → D opens Edit Colors from the visible ribbon
keytips. Pasting `color(srgb-linear 0.21404114 0.05087609 0.01444384 / 50%)`
sets RGB 127/64/32 and alpha 128. Add stores that color in slot one. Cancel
restores black, and F5 followed by slot recall restores exact `7F402080`.
The new application and browser-tab icons are present. Captures were visually
inspected: `/tmp/paint10-browser-package-{editor,linear,persisted}.png`.
This supplements the broader native and mutable-browser tests below with
actual input against the installed Nix output.

## September 8: color/icon Nix package builds

Immutable source `6baab78` builds both updated packages from the same captured
source `/nix/store/jqkw0871k091lis09fsdk11dnz0a9p6l-paint-10-source`:

- Native: `/nix/store/a3381xh695c0s0bb7l443jpxdgvwb397-paint-10-0.1.0`.
  All 350 release tests pass. Host flake checks and both-system evaluation pass.
  All 111 included files match the commit; regenerated PNG/ICO assets match,
  the committed PNG is embedded in the installed binary, and the launcher,
  desktop assets and 179-path runtime closure pass their checks.
- Browser: `/nix/store/d6ybwkv413k513rsrr2xp7va15jr4a2k-paint-10-web-0.1.0`.
  All 101 audited runtime/build/asset files match the commit. HTML, imported
  JavaScript and three favicon assets match; eight archived adapter tests pass.
  Its WASM is 10,077,645 bytes, SHA-256
  `5618d523f1f540157219a1ae822e3cff7d414b8282ac0d4e38625de7a249acbf`.

Exact derivations, manifests and qualifications are in
`tmp/{native,browser}-package-6baab78-handoff.md`. ARM Linux was evaluated only;
Windows/macOS were not executed. The browser derivation disables its native
Cargo test phase; the shared native and adapter gates are separate evidence.
Actual GUI replay of both installed outputs now passes; the native painting
and September 9 packaged-browser records below and above give the details.

The mutable browser build separately passes strict WASM Clippy, release build,
eight adapter tests and source/static/HTTP checks for all served assets at
port 8080. Logs: `tmp/browser-color-icon-{node,clippy,release,static,http}-gate.log`.
Actual F5 loads the new titlebar and tab icons; the Edit Colors dialog shows
all 48 basic and 16 custom swatches. Clipboard-pasting `rebeccapurple` and Add
sets RGB 102/51/153, stores slot one and updates Home. Pasting
`oklch(70% .4 30 / 50%)` shows authored coordinates, alpha 128 and the gamut
warning. Captures, all visually inspected:
`/tmp/paint10-browser-color-{loaded,open,named-add,oklch}.png`.

Browser Fit produces the same FF655180 as native and Add stores it in slot nine.
Pasting `rgba(10 none 30 / none)` sets alpha zero while retaining RGB 10/0/30;
Add stores that in slot two. Cancel restores black. After actual F5, Home
retains all three recent colors and recalling slot two shows exact 0A001E00,
including the RGB beneath zero alpha. The purple and fitted colors remain in
their original slots. These actual captures were visually inspected:
`/tmp/paint10-browser-color-{fit-add,none-add,cancel,reloaded,recalled}.png`.

Resizing the actual Chromium window to 550×580 keeps the dialog title, exact
entry and acceptance buttons visible. Its coordinate menu displays all eight
choices. Selecting HSV coordinates and the HSV visual picker, entering alpha
128 and clicking the saturation/value plane updates the preview and fields
while retaining that alpha. Captures, all visually inspected:
`/tmp/paint10-browser-color-{compact,compact-modes,hsv-picker-menu,hsv-picked}.png`.

The installed native `6baab78` package reopened the latest saved landscape with
matching appearance and the new icon; exact color entry then prepared a Solid
Polygon foliage pass. Captures `/tmp/paint10-package-6baab78-landscape.png` and
`/tmp/paint10-package-pine-setup.png` were visually inspected. Manual painting
continues in that installed package. The immutable browser package is served
separately at `http://127.0.0.1:8082/` for its next replay.

## September 8: full color palette and compact native replay

Commit `6baab78` restores the 48 basic and 16 custom dialog colors established
by the period-video audit, and adds the coordinate-mode editor. The combined
gate passes **350 tests (139 library + 211 app)**, strict Clippy, formatting
and the debug build in `tmp/color-editor-palette-final-native.log`. The final
readability-only delta preserves expression behavior and exact tooltip text;
scoped formatting and the full formatting check pass afterward.

The actual native GUI adds `rebeccapurple` as RGB 102/51/153 in slot one, then
stores `#33669980` in the selected sixth slot. The first slot remains intact,
the sixth displays alpha, the insertion marker advances, and Home immediately
shows the new recent swatches. Read-only private preference inspection confirms
all 16 slots and exact RGBA values. Cancel restores opaque black without
changing the saved landscape. Captures were visually inspected:
`/tmp/paint10-color-palette-native-{open,named-add,slot-six,cancel}.png`.

At 500×400, the title, Color text, status and OK/Cancel remain visible. Mouse
wheel scrolling exposes the lower palette rows while keeping the footer fixed.
Typing `oklab(50% 1e308 1e308)` and pressing Enter retains the last valid preview
and keeps acceptance disabled. Captures, all visually inspected:
`/tmp/paint10-color-palette-native-{400,400-invalid,400-scrolled}.png`.
The earlier clipped comparison remains
`/tmp/paint10-color-editor-native-400-before.png`.

Closing and relaunching the real executable within the same private desktop
preserves the Home recent list and both custom slots. Recalling slot six shows
RGB 51/102/153, alpha 128 and hex 33669980; the purple first slot remains intact.
The actual restart and recalled-editor screenshots were visually inspected:
`/tmp/paint10-color-palette-native-{restarted,recalled}.png`. Browser replay
and the landscape exercise remain unfinished. A further actual 500×500 replay
shows all 48 basic and 16 custom cells, the coordinate fields, previews and
footer without scrolling; `/tmp/paint10-color-palette-native-500.png` was
visually inspected.

## September 8: expanded color engine and first native editor replay

Commits `d2e4129` and `d6942e4` provide RGB, Paint HSL, HSL, HSV, linear RGB,
approximate CMYK, OKLab and OKLCH conversions plus literal CSS/hex parsing.
Reference values and a representative RGB cube pass; independent probes covered
100,000 colors per space and 21,816 gamut-fit cases. Those probes exposed the
near-red Paint hue wrap, and parser review caught retained negative OKLCH chroma.
Both corrections have passing regressions. Exact sources and probe qualifications
are in `tmp/color-review/review.md`.

The first integrated editor snapshot passes 133 library and 209 app tests,
formatting, strict Clippy and a native debug build:
`tmp/color-editor-final-native-gate.log`. It includes the confirmed font/File
picker corrections in `5336e3f`, whose before/after evidence is preserved in
`tmp/command-picker-fix-handoff.md`.

Actual private native input enters RGB 128, 64, 32 with successive Tab presses
and shows exact hex 804020. Switching to OKLCH retains the color. Pasting
`oklch(70% 0.4 30 / 50%)` shows the authored coordinates, alpha 128, checkerboard
preview and an out-of-sRGB notice. Clicking Fit reduces chroma to approximately
0.19158, preserves alpha, and yields FF655180. The actual titlebar also displays
the new palette-and-brush icon. Captures:
`/tmp/paint10-color-editor-native-{start,rgb,spaces,oklch,css-gamut,fitted}.png`.
All were visually inspected. Further compact-layout, palette and browser work
is in progress; this snapshot is not the final color-editor acceptance gate.

Typing `oklch(unfinished` and pressing Enter retains the last valid preview,
shows an error and keeps OK/Add disabled. Cancel then restores opaque black
without modifying the saved landscape. Actual 500×400 replay confirms the
first editor's title and footer are clipped; a bounded control area and revised
palette layout are being verified. Captures, all visually inspected:
`/tmp/paint10-color-editor-native-{invalid,cancel,400-before}.png`.

Commit `dd11e6a` adds all 148 fixed CSS names, standalone modern `none`
components, strict CSS number syntax and conversion-overflow rejection. All
13 focused color tests and strict native library Clippy pass. The name table
was checked against both hex and decimal RGB columns of the dated W3C source;
see `tmp/color-literals-handoff.md` for source and verification evidence.

Commits `ea50c8c` and `d798268` supply the new icon and native packaging.
The icon was visually inspected at application and small titlebar sizes.
Four packaging tests and actual Linux archive/extracted-helper startup pass.
The Windows resource compiler probe verifies seven exact ICO image payloads
and produces an x86-64 COFF resource object. It does not execute Windows.
macOS bundle structure is fixture-tested; actual macOS iconutil and execution
remain CI checks, not local results. See `tmp/native-icon-packaging-handoff.md`.

## September 8: shape picker stays in place when clicked

The shape gallery's default content-drag behavior used the whole input frame's
pointer movement, including motion before the button press. Moving upward from
a Size preset to Polygon could therefore scroll the gallery by one 25px row.
Content dragging is disabled for the gallery; arrow, wheel and keyboard scrolling
remain available. Actual-frame tests cover separate/coalesced presses at 1.0 and
1.05 display scales, plus the drawing workflow and arrow/wheel navigation.

The native gate passes 321 tests, strict Clippy, formatting and the debug build
(`tmp/gallery-scroll-final-native-gate.log`). The refreshed WASM build also passes.
Actual private-browser replay of Size → 3px → Polygon now keeps the complete
first row in place: `/tmp/paint10-browser-gallery-fix-size-to-polygon.png`.
The earlier failing comparison is `/tmp/paint10-browser-gallery-size-to-polygon.png`.
Both screenshots were visually inspected. Related font/File pickers are being
corrected separately after reproducing the same mechanism in their real widgets.

## September 8: final native and web Nix builds

The immutable source commit `77c1431` builds both final package snapshots:

- Native: `/nix/store/9dfkmb7khssnqicb3j7z2m5r4gydj2j1-paint-10-0.1.0`.
  All 319 release tests pass. Host flake checks, both Linux output evaluations,
  installed desktop/icon assets, runtime closure and launcher checks pass.
- Browser: `/nix/store/wga2a0qyjbpgch4yacbj93vn49b6z2a0-paint-10-web-0.1.0`.
  The captured 81 source/build/asset files and packaged HTML/JavaScript match
  the commit. The WASM is 9,532,818 bytes, SHA-256
  `3a58299d947c9be7d39d813185c0e3a141a785f48309758189e2762c4170fe3c`.
  All eight adapter tests pass against the same archived source.

Exact derivations, source hashes and logs are recorded in
`tmp/{native,browser}-package-77c1431-handoff.md`. ARM Linux was evaluated only;
Windows/macOS were not executed. The browser package's native Cargo test phase
is disabled; the 319 shared native and eight adapter tests are separate gates.

The installed native package reopened the saved landscape at 50%, and actual
Line-tool painting is continuing in that package. The packaged browser site
is served at `http://127.0.0.1:8081/`. Actual Alt, H, W, C followed immediately
by 137 and Enter produces a 137px custom size, confirmed by reopening Size.
Captures: `/tmp/paint10-package-77c1431-landscape.png` and
`/tmp/paint10-browser-package-custom-confirm.png`.

The packaged browser also draws a Polygon, applies two quick Enter presses,
and changes the next color without recoloring the applied triangle. One Undo
restores the clean blank picture. Captures:
`/tmp/paint10-browser-package-polygon-{order,undo}.png`. This replay exposed
an unwanted gallery scroll that clips the first row after selection; its
cause is being investigated separately from the passing drawing behavior.

The later test-only commit `e0b4ccd` disables Xvfb screen blanking. A fresh
private server reports timeout 0 (`tmp/private-display-no-blanking.log`).
Editing that launcher while its earlier shell was waiting for Paint caused
the old shell to report a parse error on return; the new launch passes and
the saved project reopens. This is a test-harness event, not an app crash.

## September 8: polygon completion and browser Alt replay

Commits `ed20826` and `77c1431` pass **319 native tests (125 library + 194 app)**,
formatting, strict native/WASM Clippy, eight browser adapter tests, and native
debug/browser release builds. Evidence is in
`tmp/polygon-enter-final-native-gate.log` and `tmp/browser-alt-*-gate.log`.

Actual native replay draws a triangle, sends two Enter presses 100 ms apart,
and immediately clicks the red palette swatch. The triangle is applied in its
original black with no adjustment handles; one Undo removes it and restores
the clean saved landscape. Captures:
`/tmp/paint10-native-polygon-fast-enters-{palette,undo}.png`.

Actual browser replay confirms a standalone Alt tap opens the root keytips,
then H reaches Home. Alt also works while editing text; Escape restores the
caret and subsequent typing appends to the same text. Alt+H still opens Home,
and F10 closes keytips without moving focus to Chromium's menu. Captures:
`/tmp/paint10-browser-alt-{bridge-tap,bridge-home,text-keytips,
text-focus-restored,chord-preserved}.png` and
`/tmp/paint10-browser-f10-toggle-off.png`. All were visually inspected on
separate private Xvfb displays. Updated Nix package builds are underway.

## September 8: ribbon package and actual version 2 saves

Source `e5248af` passes **316 native tests (125 library + 191 app)**,
formatting and strict Clippy. The Nix native package
`/nix/store/wrbqk46x08dxbq96crgvazdjg1vmx0q5-paint-10-0.1.0` passes those
same release tests, host flake checks, all-system evaluation and installed
asset/closure/launcher checks. ARM outputs were evaluated, not built;
Windows/macOS execution remains unverified. Exact source and evidence are in
`tmp/native-package-e5248af-handoff.md`.

Actual native mouse and keyboard replay confirms:

- Fast F10, H, W, C followed immediately by 137 and Enter preserves 137px.
  Ctrl+Plus increases 137px to 138px, within the expanded supported range.
- Select's connected dropdown opens the sectioned menu, with unavailable
  Invert/Delete disabled. Brushes opens the four-column gallery. After choosing
  Watercolor and switching to Select, clicking the main Brushes region restores
  Watercolor without opening the gallery.
- File hover shows the five distinct format previews with readable descriptions.
- Canceling custom rotation keeps the adjustable rectangle. Applying 90 degrees
  changes its bounds from 283×213 to 213×283 while the canvas remains 900×600.
  One Undo restores the rectangle and the next restores the clean blank canvas.

Captures use `/tmp/paint10-native-final-{fast137,connected-select,
connected-brushes,main-retains-watercolor,format-previews}.png` and
`/tmp/paint10-native-active-rotate*.png`. Root visually inspected these outputs.

The actual native Open and Ctrl+S workflow rewrote the ant slide as version 2.
Save a copy then re-exported its PNG to the existing destination while keeping
the project filename. Read-only comparison against the preserved version 1
project confirms exact base/composite pixels, editable objects, transforms,
styles, font faces, DPI and monochrome state. The fresh PNG is byte-identical
to the previous PNG and actual project encoder output. The project is now
1,992,405 bytes instead of 39,341,235. Evidence:
`tmp/ant-v2-gui-export-check.log` and `/tmp/paint10-native-ant-v2-saved.png`.

The matching browser release/package build passes, but actual F10 replay found
Chromium also focuses its own menu and loses following keytip letters. A narrow
browser event cancellation fix is underway; these package results do not
certify that interaction. Final browser verification follows the correction.

Commit `d23c7e8` corrects F10's browser focus conflict. After reloading the
rebuilt site, the actual 80 ms F10/H/W/C sequence followed by immediate 137 and
Enter now produces 137px. All six JavaScript adapter tests, strict WASM Clippy
and the release build pass (`tmp/browser-f10-*-gate.log`). Plain Alt is being
checked separately; Alt+H already reaches the actual Home keytips.

The browser opened the actual version 2 ant project, then Save downloaded
`/tmp/paint10-browser-ant-v2.p10`. It is byte-identical to the native project,
SHA-256 `31dbf1531a518c401e3f25dc14d63a8b61d08c68694f0f4be631b3329d810c4a`.
Double-clicking the title restores 72pt bold DejaVu Sans, and Ctrl+Enter
finishes the unchanged edit without marking the picture modified. Root
inspected `/tmp/paint10-browser-ant-v2-{downloaded,title-editor,title-no-op}.png`.

## September 8: shared fonts and version 2 projects

Commit `b9257b8` passes **313 native tests (125 library + 188 application)**,
formatting, strict native/WASM Clippy, native debug and browser release builds,
and the five browser adapter tests. Logs are
`tmp/font-sharing-all-app-tests.log`, `tmp/font-sharing-app-gate.log` and
`tmp/browser-project-v2-{node,clippy,release,static}-gate.log`.

All five actual artwork projects were decoded, encoded as version 2 and
reopened in memory without modifying their files. Comparisons preserve exact
base/composite pixels, objects, transforms, fonts, DPI and monochrome metadata.
The ant slide shrinks from 39,341,235 bytes to 1,992,405 bytes. Shared fonts
occupy 3,113,388 bytes instead of 61,969,024 repeated bytes; twelve representative
object moves retain seven undo steps within the existing memory budget.
These are codec/history measurements; actual GUI resaving remains pending.
Evidence: `tmp/project-v2-*-probe.log`. Nix packages below precede this source.

Actual native and browser text replay used exact private clipboard input:
Arabic, Hebrew, Greek, Cyrillic and an accented combining character render in
the DejaVu Sans editor. Backspace removes the whole accented grapheme; Undo
restores its original bytes. Browser double-click selects exactly the Hebrew
word, and replacement/Undo preserve logical text. Captures use
`/tmp/paint10-{native,browser}-unicode-*.png`. Unicode `xdotool type` was rejected
as a reliable test input after reproducing dropped characters in Chromium's
omnibox as well as the app; private `xclip` delivered the exact UTF-8 bytes.

Actual browser ribbon Paste also preserves the selected text after ribbon
focus, replaces it with two clipboard lines, and normalizes CRLF to LF. Copy
returns `FIRST\nSECOND` with no carriage return, and Undo restores the original
selected text. Captures: `/tmp/paint10-browser-ribbon-paste-result.png` and
`/tmp/paint10-browser-ribbon-paste-undo.png`. This replay used Chromium's allowed
Clipboard permission on a private display/profile; no host clipboard was used.

## September 8: Unicode and browser clipboard checkpoint

Commits `20b6fc5`, `fde8102` and `a0fb6b7` pass **292 native tests
(110 library + 182 application)**, two explicit vendor clipboard tests,
formatting, strict native/WASM Clippy and native/browser builds. The browser
adapter's five Node tests also pass inside the advertised `.#web` shell.
Source-specific logs: `tmp/text-unicode-word-selection-gate.log`,
`tmp/text-unicode-final-native-gate.log`, and
`tmp/browser-unicode-{node,clippy,release,static}-gate.log`.

The regressions exercise Arabic/Indic shaping, bidi caret and selection
geometry, complete grapheme deletion, Unicode word selection, ordered input,
composition cancellation and atomic undo. Actual app frames reproduce the
previous IME mode-Enabled deletion failure and confirmed-prefix loss before
their fixes. Root visually inspected Arabic, Hebrew, Indic, combining and
mixed-direction renders against the saved independent reference evidence.
Existing portrait, Mona Lisa and ant slide projects still render exactly the
same pixels as their PNG exports (`tmp/text-shaped-artwork-recheck.log`).
Native OS candidate panels have not been driven; the vendor change follows
the caret-area contract and does not claim platform input-method verification.

Fresh private native replay confirms F10, F, Down three times, Right reaches
the first PNG format row with 80 ms between keys. The final screenshot
`/tmp/paint10-native-final-complete-rapid.png` was visually inspected after
all software-rendered frames settled. Manual complex-text and shape-preview
replay is ongoing. Nix release packages below precede this newer source.

## September 8: shared native and browser verification

The source checkpoint through browser commit `f0c68b4` passes **264 native
tests (97 library + 167 application)**, formatting, strict Clippy and the native
debug build. The named Nix release package also passes all 264 tests:
`/nix/store/c59jksasbd08gh7nir2d81i6d2vwr4xh-paint-10-0.1.0`. Logs are
`tmp/native-menu-clean-gate.log` and `tmp/nix-native-final-committed.log`.
`nix flake check .` passes on x86_64-linux
(`tmp/nix-flake-final-committed.log`); incompatible aarch64 outputs were omitted
from that host check.

The browser target compiles the same Rust application. Strict WASM Clippy and
the release build pass in the pinned `.#web` shell, with Rust 1.97.1 and
wasm-bindgen 0.2.127. Logs: `tmp/wasm-final-clippy.log` and
`tmp/browser-final-release.log`. Its Nix static package also builds successfully:
`/nix/store/33n46jimrdnk4l9d95pa101slsvpqqqr-paint-10-web-0.1.0`. The packaged
HTML matches source, every generated JavaScript import exists, and the WASM
header is valid (`tmp/nix-browser-final.log`,
`tmp/nix-browser-final-static-check.log`). This package includes the final
browser production fixes; native-only debug tracing was removed afterward and
does not affect the WASM runtime.

The application regressions cover consecutive and batched File arrows, entering
the PNG format row, menu-to-editor shortcut focus, reversible shape-style
previews, exact custom brush sizes, compact Edit Colors geometry and no-op
rotated text edits. The **23 focused text tests**, included in the 264-test
total, also verify Unicode Clipboard/history at wide and 500px widths, compact
font galleries, scrolling to the last family, and registering project fonts
without changing encoded project bytes or pixels
(`tmp/font-gallery-project-fonts.log`).

Actual browser tests used private Chromium on Xvfb with a private profile,
loading the release site at `http://127.0.0.1:8080/`. Mouse/keyboard actions and
download dialogs ran inside that private desktop. The final screenshots and
rendered PDF below were visually inspected.

- **Theme and rulers:** a late browser system-theme event originally replaced
  Paint's square menu styling with egui defaults. The refreshed build retains
  the intended styling; the actual-button regression passes late Light/Dark/Light
  events. Ctrl+R now toggles rulers without also reloading the page
  (`/tmp/paint10-browser-ctrl-r-fixed.png`). F5 remains available for reload.
- **Downloads and discard protection:** an actual pencil stroke was downloaded
  as PNG. Canceling the first destination dialog retained the modified state
  and the browser's reload warning
  (`/tmp/paint10-browser-canceled-download-guard.png`). Subsequent Ctrl+S correctly
  opened the destination dialog directly after the filename was retained.
  Successful exports `/tmp/paint10-browser-first-download.png` and
  `/tmp/browser-pencil-september8.png` were visually inspected and byte-identical.
  File → New after opening a `.p10` project reset Save to PNG
  (`/tmp/paint10-browser-menu-new-png.png`).
- **Fonts and retained text:** importing `DejaVuSans.ttf` succeeded. Reopening
  the Mona Lisa project exposed its embedded DejaVu Serif family in the compact
  font gallery, with the final Load font row visible and no large blank area
  below it (`/tmp/paint10-browser-final-font-gallery.png`). Escape followed by
  Ctrl+Enter closed the gallery and committed the rotated title without
  reopening the menu or marking the unchanged project dirty
  (`/tmp/paint10-browser-final-font-commit.png`).
- **Clipboard permissions and keyboard paste:** blocking the ribbon Paste
  permission request left the document unchanged and displayed the denial
  explanation. Paste options → Paste copied selection explicitly inserted the
  locally copied image (`/tmp/paint10-browser-paste-local-fallback.png`). The
  initial silent Ctrl+V failure came from eframe stopping paste propagation at
  its document text listener. With image capture handled before that listener,
  actual Ctrl+C/Ctrl+V pasted the selected title even while Clipboard API access
  remained blocked (`/tmp/paint10-browser-final-capture-paste.png`). One Undo
  removed the pasted image and returned the project to its clean state
  (`/tmp/paint10-browser-final-paste-undo.png`).
- **Browser PDF:** Page setup, landscape orientation and print preview rendered
  the actual shape landscape. Download PDF produced
  `/tmp/paint10-browser-landscape.pdf`: one landscape Letter page, **792 × 612
  points**. Its rendered page `/tmp/paint10-browser-landscape-page.png` was
  inspected for content, orientation and placement. This verifies the export
  workflow; it does not certify the landscape's separate artwork acceptance
  requirement.

These results remain bounded. Browser downloads cannot confirm completion of
the destination dialog, so they retain the unsaved-work guard. Chrome reserves
Ctrl+N for a browser window and Ctrl+Page Up/Down for tab navigation; use File →
New and the View/status zoom controls for the picture. The
[browser guide](web/README.md) records these limits and the local font/file and
clipboard workflows. This pass does not claim manual browser coverage of every
export format or every drawing-tool combination.

Windows/MSVC and Apple/Darwin dependency trees resolve with `--locked --offline`
(`tmp/native-dependencies-windows-resumed.txt`,
`tmp/native-dependencies-macos-resumed.txt`), but neither target was compiled or
executed on this Linux host. The native CI matrix is configured; no external CI
run is claimed. Physical hardware integrations remain untested. Artwork
acceptance is tracked separately in `artworks/README.md` and `PROGRESS.md`.
After this checkpoint, manual Edit Colors reopening exposed a stale Red numeric
field when switching from an edited Color 1 to white Color 2. Commit `ce9fa9f`
clears the prior numeric draft before initial modal focus is assigned. The
regression reproduced displayed Red `0` alongside numeric value 255, then
verified all reopened RGB/HSL fields, Hex, subsequent editing and Cancel.
All 11 dialog and 12 Properties tests, strict Clippy and the refreshed native
build pass (`tmp/color-reopen-fixed-gate.log`). Actual native replay now shows
Red/Green/Blue 255 and Hex FFFFFF after editing Color 1, with the project clean
(`/tmp/paint10-ant-fixed-color-reopened.png`).

The refreshed native Nix package after `ce9fa9f` is
`/nix/store/64a87iz71vf7wr3xxsqmipxrsywv7mxg-paint-10-0.1.0`; all **265 release
tests (97 library + 168 app)** and flake checks pass. The matching refreshed web
package is `/nix/store/jsgw1a4xm6sqv6ym33x36i0yy729jk01-paint-10-web-0.1.0`;
WASM strict Clippy, release and static checks pass.

Additional actual browser checks:

- At a narrow window, Home/View groups and their menus fit. Measuring A(9,9)
  to B(12,13) gives 5.00px and 53.13 degrees. Right then Shift+Right moves B to
  (23,13): 14.56px, 15.95 degrees and 3.852mm at 96dpi. All readout fields fit,
  and the project stays clean. Captures use
  `/tmp/paint10-browser-final-measure-five500.png` and `-measure-mm500.png`.
- The Mona Lisa project was opened and actually downloaded again as `.p10`.
  Read-only comparison with `tmp/check-browser-project.py` verifies identical
  image bytes, objects, transforms, fonts and other project values. The newer
  serializer explicitly writes three empty `font_faces` defaults omitted by the
  older source project, so the compressed files are not byte-identical.
  Download: `/tmp/paint10-browser-mona-roundtrip.p10`.

Earlier results below retain their original source snapshots and scope.

## Responsive ribbon, captions, transparency and portability pass

Ordinary verification for source commit `32ff8ec` passes **197 tests (84 library + 113 application)** in both the native suite and the named Nix release package. Strict Clippy, formatting, the native build and x86_64-linux flake check pass. The two vendored clipboard tests also pass when run explicitly; CI now includes that command. Windows/macOS native execution remains unverified locally. The five final artwork acceptance exercises have begun as a separate pass and are not counted in these results.

Verified release package: `/nix/store/jrp82pfckf8kdqnw0y8klnfb2nzs648b-paint-10-0.1.0`. Build log: `tmp/nix-package-verified.log`. The live application sources matched the package's source snapshot. The ARM Linux package output evaluates, but was not built or executed on this host.

- **Large outlined captions:** the first live 54-point caption had mismatched foreground/outline glyphs (`/tmp/paint10-caption-54-live.png`). The editor and raster renderer used different font-size units. Live glyph pixels and caret geometry now come from the document renderer. Actual typing, white/bold/centered/outlined formatting, word replacement and Ctrl+Enter produce matching live and committed text (`/tmp/paint10-caption-live-shared-fixed.png`, `/tmp/paint10-caption-edited-shared.png`, `/tmp/paint10-caption-committed-shared.png`). App-frame regressions compare the texture in the typing frame with the committed raster and click within centered glyphs at 100%/400% zoom.
- **500px ribbon:** the native minimum width now exposes collapsed groups, all tool commands and a readable zoom/status area (`/tmp/paint10-ribbon-real-500.png`). Keyboard ZK opens the palette with its anchored keytips (`/tmp/paint10-500-color-keytips.png`). Switching to Shapes closes Colors, leaving a single popup (`/tmp/paint10-500-sibling-groups.png`). This exposed clipped Edit colors text; the final `jrp…` package replay confirms anchored keytips (`/tmp/paint10-final-500-colors.png`) and the fully wrapped, unobstructed label (`/tmp/paint10-final-500-colors-label.png`). Both final screenshots were visually inspected.
- **Keyboard event failures:** real-frame tests reproduced selected text being replaced by a synthetic Enter, stale popup ancestry after resize, mouse-menu Arrow/Escape failures, fast multi-digit font input and numeric Escape committing an unwanted draft. The controller tests now pass all those cases using actual widgets.
- **Native Command shortcuts:** input-frame tests cover macOS Command and physical Ctrl for document commands, text formatting/history, Select all and wheel zoom. Cmd+W/Cmd+Q close through the unsaved-change guard; physical Ctrl+W retains Paint's Resize and skew command. The explicit egui-winit tests cover clipboard recognition and physical Ctrl normalization. These are shared input tests on Linux, not a claim of a native macOS run.
- **Transparency and geometry:** shared alpha compositing avoids dark speckles and opacity loss. Library/gesture tests cover transparent editable objects, transparent Fill and eraser undo, exact RGBA preservation, Color 2 beneath newly exposed off-canvas objects, line endpoints and tight cubic bounds. Fill with transparent or translucent Color 2 merges visible objects into the filled raster when pixels change; one Undo restores the pixels, editable objects and saved state. No-op fills preserve the objects. The caption fixture remains byte-identical after adding shared editing geometry.
- **Stroke release boundary:** a real-frame regression reproduced Pencil/Eraser extending a finished click when the same frame also contained later hover motion. Gesture endpoints and collected stroke motion now stop at release. Separate-frame and coalesced press/release cases pass, including pixel and Undo checks.
- **Saving and portability:** codec tests cover exact RGBA and indexed palettes. App tests cover Save a copy/selection without changing the current destination, including an object wholly outside the canvas. All eleven choices and Cancel are exercised in the portable format picker. A real subprocess regression verifies Save As can launch after the running development executable has been replaced. Both NixOS consumption examples evaluate to the same package; the native CI matrix is checked in but has not run externally.
- **Pixel-art exports through the real GUI:** saved the 32×24 project `/tmp/paint10-pixel-probe.p10` and PNG copy `/tmp/paint10-pixel-probe-copy.png`; every RGBA byte matches. Resizing with nearest-neighbor interpolation produced `/tmp/paint10-pixel-scaled.png` at 64×48, with every source pixel replicated into an exact 2×2 block. All three original RGBA colors remain, with alpha values only 0 and 255. Save selection as produced `/tmp/paint10-pixel-selection.webp`, an exact 18×18 RGBA crop of the selected region. The read-only `tmp/check_pixel_exports.py` checks these assertions and passed again while documenting this evidence. Reopening the scaled PNG retains its checkerboard transparency (`/tmp/paint10-transparent-reopened-settled.png`, visually inspected).

## Earlier reopened audit after the completion challenge

The earlier 76-test pass was insufficient. `PARITY_AUDIT.md` records the concrete failures found afterward and the remaining differences. The following checks were performed against rebuilt native executables, using real mouse and keyboard input on private Xvfb desktops.

- **Clipboard replacement and text completion:** reproduced the old stale-image paste after copying characters (`/tmp/paint10-stale-clipboard-confirmed.png`). The rebuilt app finishes text with Ctrl+Enter and reports no picture on a text-only clipboard, preserving the drawing (`/tmp/paint10-audit-clipboard-fixed.png`). An unsuccessful image paste also keeps active text open in a focused regression.
- **Text palette and clipboard:** only `second` changed color in `First second` (`/tmp/paint10-audit-word-blue-settled.png`). Home Copy/Cut removed only the selected characters; Quick Access Undo restored the word and its color (`/tmp/paint10-audit-home-cut.png`, `/tmp/paint10-audit-text-qat-undo.png`).
- **Font entry:** typed `4` and `6` in separate rendered frames, then Enter; the font became 46 points without inserting digits into the text (`/tmp/paint10-audit-font-46.png`). Typed `DejaVu Serif` into the new font-name field for one selected word. Its changed rendering is visible in `/tmp/paint10-audit-thumbnail-navigated-text.png`. The automated font test constructs a real two-face TTC, selects face 1, compares its raster against the standalone font and round-trips the face through `.p10`.
- **Active text handles:** resized the typing box until two words reflowed onto separate lines, moved its border, committed and reopened it at the new position (`/tmp/paint10-audit-text-handle-wrap.png`, `/tmp/paint10-audit-text-handle-move.png`, `/tmp/paint10-audit-moved-text-reopened-settled.png`). Actual-frame tests also check geometry undo/redo.
- **Transformed text:** a 90° text rotation produced 130×205 bounds. Touching its handle preserved both pixels and the saved title; a horizontal drag stretched it to 230×205 without reflow (`/tmp/paint10-audit-noop-stays-saved.png`, `/tmp/paint10-audit-rotated-text-stretched.png`). Saved through the native `.p10` format selector and reopened with the transform intact (`/tmp/paint10-audit-project-reopened.png`).
- **Off-canvas resize:** moved that object partly outside the canvas. The status reported visible bounds of 153×205 while Resize correctly initialized the full 230×205 dimensions (`/tmp/paint10-audit-offcanvas-resize-dialog.png`).
- **Stroke clipping:** an out-and-back brush drag left two clipped diagonal segments and no unwanted horizontal stroke along the canvas edge (`/tmp/paint10-audit-outside-brush.png`). In the packaged app, copying and pasting an active 8-pixel horizontal line retained all eight rows, with 208×8 pasted bounds (`/tmp/paint10-audit-thick-line-paste.png`).
- **Quick Access Toolbar:** added Open, moved the toolbar below the ribbon, inspected its private preferences file, closed and reopened the app, and verified both settings persisted (`/tmp/paint10-audit-qat-persisted.png`). Manual testing exposed missing direct Alt-number handling; after fixing it, customized Alt+4 opened the native Open dialog in the final package (`/tmp/paint10-audit-packaged-alt4.png`).
- **Thumbnail and keyboard context:** at 200%, enabled Thumbnail while editing text, then clicked its preview to scroll the active text box into view without committing it (`/tmp/paint10-audit-thumbnail-navigated-text.png`). Shift+F10 displayed text-specific commands (`/tmp/paint10-audit-text-context-menu.png`). The Thumbnail regression was verified to fail without the overlay-layer guard and pass with it using the actual canvas → thumbnail → dialogs update order.
- **Drawing and gallery regressions:** actual-frame checks cover stationary marker opacity, brush continuation across event batches, curve stability between bend drags, the polygon's first-edge preview and no-op selection handles. Raster checks cover all 23 shape bounds at widths 1/3/8/50, noncrossing Cloud callout geometry and rounded callout corners. The regenerated raster/contact sheets were visually inspected. The 500px ribbon regression checks keyboard scrolling, activation and accessible names in both gallery presentations.

Historical verification for that earlier audit: **122 tests passed (62 library, 60 application)** in its Nix release package. Native build, strict `cargo clippy --all-targets -- -D warnings`, formatting and `nix flake check .` passed on x86_64-linux. Historical release log: `tmp/final-package-build.log`. Package tested then: `/nix/store/pc6vxlmfsvn05v0j0fwqbj4midwbbywd-paint-10-0.1.0`.

The live development session used `/tmp/paint10-desktop.kWQhCQ`; the final packaged session used `/tmp/paint10-desktop.z1i8U7`. The software renderer can lag input; screenshots above were inspected after the relevant state settled. The early 45 ms-per-digit font-entry attempt on the old binary did **not** reproduce the focus bug and is not cited as failure evidence.

The earlier passes below are historical evidence, not proof that the original requirement was already complete. Hardware/portal checks and exhaustive Windows equivalence remain unverified; see `PARITY_AUDIT.md`.

## Environment

- Native Rust eframe/egui 0.31.1, Nix dev shell, Rust 1.97.1.
- The shared KDE Wayland desktop locked during implementation. Computer Use screenshots confirmed the lock screen. No session settings were changed.
- Xvfb displays (1280×900) are used for manual mouse/keyboard tests: initially `:99`, then dynamically allocated `:1` and `:2` through the delivered launcher. Screenshots use the screenshot skill helper. Actions use XTEST/xdotool against the actual application window.
- Isolation correction: GTK initially auto-discovered the host Wayland socket and opened Save on the user's desktop. The test process was stopped. The launcher now forces `GDK_BACKEND=x11`, creates a private runtime directory, and uses `dbus-run-session`. A new screenshot of Xvfb itself confirms the native Save dialog is contained there: `/tmp/paint10-truly-isolated-save.png`.
- The user's two-hour desktop restriction expired at 2026-09-07 05:07:54 UTC. The reopened audit continued using private desktops.
- Mesa 26.1.8 llvmpipe stalled in `lp_fence_wait` on the isolated display. GDB stacks localized it to the software rasterizer. The isolated test uses `GALLIUM_DRIVER=softpipe`; the application does not force a graphics driver or disable vsync.

## First manual pass

- Initial ribbon, palette, canvas, title and status render. Screenshot: `/tmp/paint10-softpipe.png`.
- Rectangle outline and fill work; fill remains inside the border. Screenshot: `/tmp/paint10-fill.png`.
- Ctrl+Z removes a committed text insertion, Ctrl+Y restores it. Screenshot: `/tmp/paint10-undo-focused.png`.
- In-canvas text entry and contextual font ribbon work. Screenshot: `/tmp/paint10-text-focused.png`.
- Fixed missing dropdown glyphs.
- Fixed fast drags beginning at the latest cursor position. Retested stroke and rectangle start/end coordinates: `/tmp/paint10-drag-retest.png`.
- Fixed double-clicking unselected text when clicks coalesce into one frame. Reopening text verified: `/tmp/paint10-double-fixed.png`.

## File dialog pass

- Found a GTK crash when saving because GSettings schemas were absent. Fixed the Nix environment and package wrapper.
- Saved `/tmp/paint10-native-save.png` through the native dialog with keyboard input; returned to the canvas with the updated title and saved status: `/tmp/paint10-save-complete.png`.

## Rich text and clipboard pass

- Typed `Hello world`, selected only `world` with Ctrl+Shift+Left, and applied Ctrl+B/Ctrl+I. Only the selected word changed: `/tmp/paint10-word-styles.png`.
- Ctrl+Z removed italic while retaining bold and the text: `/tmp/paint10-style-undo.png`. Ctrl+Y restored italic.
- Committing text retained both styles in the raster rendering. Copied the selected object and pasted it with Ctrl+V; a second visible image appeared at the canvas origin: `/tmp/paint10-image-paste-fixed.png`.
- This exposed an egui-winit 0.31 input limitation: it consumed image-only paste shortcuts without emitting a Paste event. A minimal vendored patch now preserves the request; the unreliable key-release fallback was removed.
- The actual page preview rendered the test oval within the configured page: `/tmp/paint10-print-preview.png`.
- Closing the scratch document displayed Save / Don't save / Cancel: `/tmp/paint10-close-prompt.png`.

## Automated checks

- Initial 7 tests passed: fill boundaries, history/saved state, canceled previews, clipped paste, rotation/canvas sizing, text/PNG round-trip, PDF layout.
- The earlier integrated suite passed 76 tests (54 library and 22 UI tests), including image metadata, bitmap bit depths, rich text/history, transformed selections, project bounds, custom paper sizes and modal keyboard behavior. Strict `cargo clippy --all-targets -- -D warnings` and formatting checks passed. The three dialog keyboard tests use an actual modal Window, and passed again after the manual focus fix.
- Twelve printing tests pass. Letter, A4 landscape with asymmetric margins, Legal, A3 landscape, custom 100×160 mm paper, and every page of two four-page tiled PDFs were rendered with Poppler and visually inspected. Fixtures are in `tmp/pdfs/`.

## Manual coverage

The passes below cover drawing and fill, rectangle/ellipse/curve/polygon, rich text, selected-object movement/resizing/rotation, clipboard, native save/reopen, print preview and PDF export, fullscreen, keyboard navigation, modal entry, rulers/grid/zoom and small-window layout. Every brush/selection combination has not received an exhaustive manual workflow pass.

## Rebuilt app and icon pass

- The delivered `scripts/headless-desktop.sh` was used to launch the real app on dynamically allocated Xvfb display `:1`, with Openbox, private runtime/config/data directories and a private D-Bus session. This verifies the advertised test-shell entrypoint.
- Reopened `/tmp/paint10-richtext.p10`; both the original styled text and the independently resized pasted image render correctly: `/tmp/paint10-new-ui.png`. The older binary had displayed a stale preview; the project data and current library rendering were intact.
- Alt+H, H, 4 selected Rectangle using keyboard commands: `/tmp/paint10-keytips.png`.
- A released rectangle retained its draft handles. Moving, resizing, outline recoloring and textured fill all updated the same shape: `/tmp/paint10-shape-draft.png`, `/tmp/paint10-shape-moved-color.png`, `/tmp/paint10-shape-resized.png`, `/tmp/paint10-shape-textured.png`. One Ctrl+Z removed the entire adjusted shape; Ctrl+Y restored it.
- Dragging both curve bends produced an adjustable S-shaped curve: `/tmp/paint10-curve-first.png`, `/tmp/paint10-curve-complete.png`.
- F11 occupied the full isolated screen and Escape restored the editor: `/tmp/paint10-fullscreen.png`.
- Ctrl+Shift+S opened the new native Save As helper. Its dropdown exposed all formats and bitmap depths. Selecting 16-color BMP produced `/tmp/paint10-export-16.bmp`: Windows 3.x BMP, 900×600, **4 bits per pixel**, with 96 DPI metadata. Screenshots: `/tmp/paint10-format-options.png`, `/tmp/paint10-bmp-name.png`, `/tmp/paint10-bmp-saved.png`.
- The icon subagent rendered and inspected the actual vector artwork at ribbon sizes, all 23 shapes, nine brushes/stroke samples, and selected/disabled/focused button states. Contact sheet: `tmp/icons-contact-sheet.png`. AccessKit checks verified names and states. Final ribbon inspection: `/tmp/paint10-final-ui.png`.
- Manual Resize testing caught initial focus being consumed by egui's invisible window-layout pass. The test was changed from a generic panel to an actual modal Window, which reproduced the failure. After the fix, Ctrl+W selected the width; typing `600` and pressing Enter resized 900×600 to 600×400; Ctrl+Z restored the original image. Screenshots: `/tmp/paint10-resize-focus-fixed.png`, `/tmp/paint10-resize-enter-fixed.png`.
- Ctrl+R and Ctrl+G toggle rulers and gridlines. Ctrl+PageUp zooms the picture; the grid is visible at 400%: `/tmp/paint10-grid-400.png`. An 800×600 window retains the title controls, scrolling ribbon, canvas scrollbars and zoom controls: `/tmp/paint10-small-window.png`.
- At this earlier stage, `nix build . --no-link` and `nix flake check .` passed for x86_64-linux, and that package's release suite passed all 76 tests. The package wrapper includes GTK schemas, graphics libraries and capture helpers. Aarch64 was not built on this host.

## Earlier packaged workflow pass

- Launched the packaged entrypoint with `nix develop .#test -c scripts/headless-desktop.sh nix run . -- /tmp/paint10-richtext.p10`: `/tmp/paint10-nix-run.png`.
- Rotated the selected pasted image to 33° through the angle dialog. Its bounds changed from 420×70 to 391×288; the separate editable text stayed unchanged. Ctrl+Z restored the image: `/tmp/paint10-rotate-33.png`.
- Drew the polygon's first edge, added vertices, and double-clicked to complete a four-sided adjustable shape. One Ctrl+Z removed it: `/tmp/paint10-polygon-complete.png`, `/tmp/paint10-polygon-undo.png`.
- Fixed Ctrl+Shift+N clearing only the selected object. The shortcut now clears the whole picture; one Ctrl+Z restores both objects: `/tmp/paint10-clear-selection-before.png`, `/tmp/paint10-clear-picture.png`, `/tmp/paint10-clear-undo.png`.
- Opened the paper presets and custom dimensions through Page Setup. A zero width displayed a validation error and disabled Print, Save PDF and Preview. Entering 100×160 mm restored them: `/tmp/paint10-paper-presets.png`, `/tmp/paint10-custom-invalid.png`, `/tmp/paint10-custom-100x160.png`.
- Saved `/tmp/paint10-manual-custom.pdf` through the isolated native dialog. Poppler reported one page with a 283.465×453.543-point MediaBox, matching 100×160 mm. The rendered PDF and live preview show the same content placement: `/tmp/paint10-manual-custom.png`, `/tmp/paint10-custom-preview.png`.

## Verification limits

The manual passes exercise the actual Rust application with mouse and keyboard input; they are not exhaustive equivalence tests against a Windows installation. Physical printers/scanners/cameras and host wallpaper/email portals were not invoked. The host desktop has remained untouched since isolation was corrected. Screenshots and scratch exports are temporary evidence, deliberately excluded from Git.
