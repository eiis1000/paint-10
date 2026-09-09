{
  description = "Paint 10 — a native Rust Windows 10 Paint recreation";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      eachSystem = nixpkgs.lib.genAttrs systems;
      desktopLibs =
        pkgs: with pkgs; [
          libGL
          libxkbcommon
          wayland
          libX11
          libXcursor
          libXi
          libXrandr
        ];
      captureTools =
        pkgs: with pkgs; [
          sane-backends
          ffmpeg-headless
        ];
      source = nixpkgs.lib.cleanSourceWith {
        name = "paint-10-source";
        src = ./.;
        filter =
          path: type:
          let
            name = baseNameOf path;
            excluded = [
              "target"
              "tmp"
              "artworks"
              ".git"
              ".codex"
              ".agents"
              ".direnv"
              "result"
            ];
          in
          !(builtins.elem name excluded)
          && !(nixpkgs.lib.hasPrefix "result-" name)
          && nixpkgs.lib.cleanSourceFilter path type;
      };
      mkPaint =
        pkgs:
        pkgs.rustPlatform.buildRustPackage {
          pname = "paint-10";
          version = "0.1.0";
          src = source;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [
            pkgs.wrapGAppsHook3
            pkgs.pkg-config
          ];
          buildInputs = [
            pkgs.gtk3
            pkgs.gsettings-desktop-schemas
          ];
          postInstall = ''
            install -Dm644 assets/paint-10.desktop "$out/share/applications/paint-10.desktop"
            install -Dm644 assets/paint-10.svg "$out/share/icons/hicolor/scalable/apps/paint-10.svg"
            install -Dm644 assets/fonts/DejaVu-LICENSE.txt "$out/share/licenses/paint-10/DejaVu-LICENSE.txt"
          '';
          preFixup = ''
            gappsWrapperArgs+=(--prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath (desktopLibs pkgs)})
            gappsWrapperArgs+=(--prefix PATH : ${pkgs.lib.makeBinPath (captureTools pkgs)})
          '';
          meta = {
            description = "A native Rust drawing application with the Windows 10 Paint workflow";
            license = pkgs.lib.licenses.mit;
            platforms = systems;
            mainProgram = "paint-10";
          };
        };
      mkWeb =
        pkgs:
        pkgs.rustPlatform.buildRustPackage {
          pname = "paint-10-web";
          version = "0.1.0";
          src = source;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [
            pkgs.wasm-bindgen-cli
            pkgs.llvmPackages.lld
          ];
          # Override the native Cargo hook's target while retaining its locked,
          # vendored dependency setup. Browser behavior is checked in shared tests.
          buildPhase = ''
            runHook preBuild
            cargo build --frozen --release --lib --target wasm32-unknown-unknown -j "$NIX_BUILD_CORES"
            runHook postBuild
          '';
          doCheck = false;
          installPhase = ''
            runHook preInstall
            mkdir -p "$out/pkg"
            wasm-bindgen --target web --out-dir "$out/pkg" --out-name paint_10 target/wasm32-unknown-unknown/release/paint_10.wasm
            cp web/index.html "$out/index.html"
            cp assets/paint-10.svg assets/paint-10.png assets/paint-10.ico "$out/"
            cp assets/fonts/DejaVu-LICENSE.txt "$out/"
            runHook postInstall
          '';
          meta = {
            description = "Paint 10 browser application using the shared Rust UI and engine";
            license = pkgs.lib.licenses.mit;
            platforms = systems;
          };
        };
    in
    {
      devShells = eachSystem (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        rec {
          default = pkgs.mkShell {
            packages =
              (with pkgs; [
                cargo
                rustc
                rustfmt
                clippy
              ])
              ++ captureTools pkgs;
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [
              pkgs.gtk3
              pkgs.gsettings-desktop-schemas
            ];
            # GLib's setup hook collects schema paths from buildInputs. Keep the
            # caller's desktop data directories when making those schemas visible.
            shellHook = ''
              export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath (desktopLibs pkgs)}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
              export XDG_DATA_DIRS="''${GSETTINGS_SCHEMAS_PATH}:''${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"
            '';
          };
          test = default.overrideAttrs (old: {
            nativeBuildInputs =
              old.nativeBuildInputs
              ++ (with pkgs; [
                xorg-server
                dbus
                xdotool
                xclip
                imagemagick
                openbox
              ]);
          });
          # nixpkgs' Rust includes the wasm32 standard library. Keep the
          # binding generator in sync with the pinned wasm-bindgen crate.
          web = default.overrideAttrs (old: {
            nativeBuildInputs = old.nativeBuildInputs ++ [
              pkgs.wasm-bindgen-cli
              pkgs.llvmPackages.lld
              pkgs.python3
              pkgs.nodejs
            ];
          });
        }
      );
      packages = eachSystem (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        rec {
          paint-10 = mkPaint pkgs;
          paint-10-web = mkWeb pkgs;
          default = paint-10;
        }
      );
      overlays.default = final: _prev: { paint-10 = mkPaint final; };
      checks = eachSystem (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          desktop-entry =
            pkgs.runCommand "paint-10-desktop-entry-check"
              {
                nativeBuildInputs = [ pkgs.desktop-file-utils ];
              }
              ''
                desktop-file-validate ${./assets/paint-10.desktop}
                touch "$out"
              '';
        }
      );
      formatter = eachSystem (system: (import nixpkgs { inherit system; }).nixfmt);
    };
}
