# Paint 10 in a browser

The browser build compiles the same Rust drawing engine and `PaintApp` interface
as the native application. It requires WebGL and a current desktop browser.

**[Launch Paint 10](https://eiis1000.github.io/paint-10/)**

## Build and run

With the repository's pinned Nix environment:

```sh
nix develop .#web -c bash scripts/build-web.sh
nix develop .#web -c python3 -m http.server 8080 --bind 127.0.0.1 --directory target/web
```

Open `http://127.0.0.1:8080/`. The output is a static site in `target/web`; no
application server or image upload service is involved. A packaged static site
is also available through `nix build .#paint-10-web`.

For disposable builds on Linux, set `CARGO_TARGET_DIR` before building:

```sh
export CARGO_TARGET_DIR=/tmp/paint-10-target
nix develop .#web -c bash scripts/build-web.sh
nix develop .#web -c python3 -m http.server 8080 --bind 127.0.0.1 --directory "$CARGO_TARGET_DIR/web"
```

Both Cargo's intermediates and the generated site then stay outside the
checkout. They can be rebuilt after `/tmp` is cleared.

Without Nix, install Rust's `wasm32-unknown-unknown` target, LLVM `wasm-ld`, and
`wasm-bindgen-cli` version **0.2.127**, then run `bash scripts/build-web.sh`.
The binding generator version must match the crate pinned in `Cargo.toml`.

## Publish with GitHub Pages

The repository's [Build and test workflow](../.github/workflows/build.yml) builds
the browser app and deploys it to GitHub Pages only after the native, Nix, and
browser checks all pass. This repository's site is configured already.
For a fork or a new repository:

1. Open **Settings → Pages → Build and deployment** and set **Source** to
   **GitHub Actions**. No generated workflow or `gh-pages` branch is needed.
2. Open **Actions → Build and test → Run workflow**, select the repository's
   default branch, and run it. Later pushes to that branch deploy automatically.
3. Open the site link in the **deploy-pages** job or **Settings → Pages**.
   A project repository normally appears at
   `https://OWNER.github.io/REPOSITORY/`.

The workflow detects the default branch instead of assuming `main` or `master`.
Pull requests, tags, and other branches are tested without deployment. GitHub's
built-in token handles deployment; no personal access token or repository secret
is needed. Until the Pages source is enabled, the deployment job reports a Pages
configuration error; the build artifacts remain downloadable from the run.
These steps follow [GitHub's custom Pages workflow documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).

The deployed artifact contains only the generated static app, icons, and bundled
font license. Relative asset URLs support project subpaths, account sites, and
custom domains without changing the build. GitHub Pages provides HTTPS, which
also makes the browser clipboard APIs available subject to user permissions.
Images and projects stay in the browser; publishing the app does not upload your
pictures.

The static HTML includes a search description, canonical URL, social previews,
and structured application metadata. When hosting a fork or changing domains,
update the absolute URLs in `web/index.html` to the intended public site.

## Files, text, and clipboard

- **Open** loads an image or editable `.p10` project. **Paste from** and dropping
  a file insert an image into the current picture. Standard browser file inputs
  work without the File System Access API.
- **Save** downloads a file. It cannot replace the original automatically.
  The browser cannot confirm that you completed its download dialog, so Paint 10
  keeps unsaved work marked as modified. If saving before New/Open, verify the
  download and then choose **Don't save** in the pending discard prompt.
  **Save as** offers PNG, JPEG, GIF, TIFF, WebP, ICO, all four Paint BMP depths,
  and editable Paint 10 projects. Save a copy and Save selection are available.
  Use `.p10` to retain layers, editable text, embedded fonts, original images, source
  crops, color adjustments, and transforms.
- **View → Layers** opens the optional layer pane. Drawing and image adjustments
  affect the selected layer; image downloads combine visible layers. Project
  downloads preserve the complete stack, including hidden layers. The browser
  and native builds use the same layer controls and project format.
- **Font list → Load font** accepts TTF, OTF, and TTC files up to 32 MiB. Select
  the loaded family in the font list. Font data used in text is embedded in a
  project. Opening a project also adds its embedded families to the font list,
  so they can be reused in other text boxes. The browser does not enumerate
  installed system fonts.
- Browser clipboard access depends on permissions and a secure context
  (`localhost` qualifies). Ctrl+V / Command+V accepts normal browser paste
  events. The ribbon Paste button also requests clipboard access. If permission
  is denied, the document stays unchanged and the status explains alternatives.
  **Paste options → Paste copied selection** explicitly uses this tab's last
  image copy, without reading the system clipboard.
- Custom colors and Quick Access preferences are kept in browser local storage.
  Pictures are not automatically saved there. Unsaved work triggers the browser's
  normal tab-close warning after interaction; download work before closing.

## Browser boundaries

Page setup and print preview use the shared layout engine. **Download PDF**
creates the same printable PDF; open that download to print through the browser
or an installed PDF viewer. Scanner drivers, email attachments, and desktop
wallpaper integration require the native application and are labeled in the UI.

Browsers reserve some shortcuts, including opening/closing tabs and parts of
their developer tools. Use the equivalent ribbon or File menu command when a
browser shortcut takes precedence. These limitations do not remove the drawing,
selection, text, image transformation, history, zoom, or export tools.

In Chrome, Ctrl+N / Command+N opens a browser window; use **File → New** for a
new picture. Ctrl+Page Up / Page Down changes browser tabs; use **View → Zoom in /
Zoom out** or the status-bar zoom controls for the picture. These reserved keys
can act on the browser before the page receives an event, as defined by
[Chromium's reserved command handling](https://chromium.googlesource.com/chromium/src.git/+/52a94675e59339721e2867435e56cae684a96d26/chrome/browser/ui/browser_command_controller.cc#395).

With the Paint canvas focused, Ctrl+R / Command+R toggles rulers without also
reloading the page. Use F5 or the browser's Reload button to reload Paint 10.

Alt or F10 opens the ribbon keytips, including while editing text. Press the
displayed letters to choose a command; Escape returns to the canvas or text
caret. Alt+H also opens the Home commands directly.
