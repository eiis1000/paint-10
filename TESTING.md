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
