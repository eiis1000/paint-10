#!/usr/bin/env bash
set -euo pipefail

# A private display, bus, runtime directory, and GTK settings keep manual GUI
# tests separate from the user's active desktop. Logs remain available afterward.
for program in Xvfb dbus-run-session; do
    if ! command -v "$program" >/dev/null; then
        printf 'Missing %s. Run this script from the Nix test shell.\n' "$program" >&2
        exit 1
    fi
done

paint_test_dir=$(mktemp -d /tmp/paint10-desktop.XXXXXX)
chmod 700 "$paint_test_dir"
mkdir -p "$paint_test_dir"/{runtime,config,cache,data}
chmod 700 "$paint_test_dir/runtime"

export XDG_RUNTIME_DIR="$paint_test_dir/runtime"
export XDG_CONFIG_HOME="$paint_test_dir/config"
export XDG_CACHE_HOME="$paint_test_dir/cache"
export XDG_DATA_HOME="$paint_test_dir/data"
export GDK_BACKEND=x11 GTK_USE_PORTAL=0 WINIT_UNIX_BACKEND=x11
export GALLIUM_DRIVER="${PAINT10_TEST_DRIVER:-softpipe}"
unset WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS

Xvfb -displayfd 3 -screen 0 1280x900x24 -nolisten tcp \
    3>"$paint_test_dir/display" >"$paint_test_dir/xvfb.log" 2>&1 &
paint_xvfb_pid=$!
paint_wm_pid=

cleanup() {
    if [[ -n "$paint_wm_pid" ]]; then
        kill "$paint_wm_pid" 2>/dev/null || true
    fi
    kill "$paint_xvfb_pid" 2>/dev/null || true
}
trap cleanup EXIT

for attempt in {1..100}; do
    if [[ -s "$paint_test_dir/display" ]]; then
        break
    fi
    if ! kill -0 "$paint_xvfb_pid" 2>/dev/null; then
        cat "$paint_test_dir/xvfb.log" >&2
        exit 1
    fi
    sleep 0.1
done

if [[ ! -s "$paint_test_dir/display" ]]; then
    printf 'Xvfb did not become ready. See %s/xvfb.log\n' "$paint_test_dir" >&2
    exit 1
fi
read -r paint_display_number <"$paint_test_dir/display"
export DISPLAY=":$paint_display_number"

if command -v openbox >/dev/null; then
    openbox --sm-disable >"$paint_test_dir/window-manager.log" 2>&1 &
    paint_wm_pid=$!
fi

printf 'Paint 10 test display: %s\nLogs and settings: %s\n' "$DISPLAY" "$paint_test_dir"
if [[ $# -eq 0 ]]; then
    set -- cargo run
fi
dbus-run-session -- "$@"
