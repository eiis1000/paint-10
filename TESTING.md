# Verification

Run the automated checks from the repository root:

```sh
nix develop .#web -c cargo test --locked --all-targets
nix develop .#web -c cargo fmt --check
nix develop .#web -c cargo clippy --locked --all-targets -- -D warnings
nix develop .#web -c node --test web/browser-events.test.mjs
nix develop .#web -c python scripts/check-repository-privacy.py --all-history
nix flake check
```

The native package was verified with 192 library and 316 application tests.
All 11 browser adapter tests passed. The tests cover document editing,
project roundtrips, image transforms, text, layers, input handling and UI
layout. Passing these checks does not establish full Microsoft Paint parity.

`scripts/headless-desktop.sh` starts an isolated Xvfb session for manual native
checks. `scripts/check-firefox-paste.py` diagnoses real browser image paste on
a private X11 clipboard; it requires Firefox, geckodriver, Xvfb and xclip.
Its reports go under ignored `tmp/`. A fresh Firefox profile successfully
inserted the test PNG; existing sessions and Wayland clipboard bridges remain
separate verification cases.

Linux and the browser have been exercised manually. Windows and macOS builds
are covered by CI, but their desktop workflows have not been manually verified.
Physical printing, scanner/camera drivers, email clients and wallpaper portals
need checks on supported host systems.

Keep personal work logs and machine-specific evidence in ignored `tmp/`.
The privacy check rejects private-note paths and home-directory paths without
restricting commit authorship. It is a targeted regression check, not a
comprehensive secret scanner.
With `uv` installed, enable the local commit/push checks using
`git config core.hooksPath .githooks`. CI also checks reachable history.

## CI cost controls

- Pushes to `main` and pull requests run CI; feature-branch pushes do not also
  duplicate their PR run. New commits cancel superseded runs of the same ref.
- Documentation-only changes run the lightweight checks. Native code changes
  run tests and Clippy on Linux, Windows and macOS; web-only changes use Linux.
- Linux native and browser builds share a job and Cargo cache. Compiled target
  code remains specific to its platform, compiler and profile. PRs restore
  these caches but do not save separate copies.
- Ordinary native checks use the development profile. `v*` tags and manual
  **full** runs test and package in the release profile, reusing dependencies
  within that profile. Formatting and Python tests run once per workflow.
- Nix package builds run for flake, dependency, build-script and workflow
  changes, plus full runs. Ordinary Rust edits rely on native Cargo tests;
  use a full run to verify those changes against the pinned Nix toolchain.
- Native archives expire after seven days. Browser previews and Pages uploads
  expire after one day; default-branch builds upload only the Pages artifact.
  Artifact expiry does not remove an already deployed Pages site.
- CI downloads the checksum-pinned official `wasm-bindgen` binary instead of
  compiling its CLI. Update its version and checksum together when updating
  the matching Cargo dependency and browser build script.
