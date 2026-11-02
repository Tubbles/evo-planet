#!/usr/bin/env bash
# Pull the latest commits, build Evo Planet, and run it in a kiosk browser,
# showing build progress on screen the whole time. Made for a TV machine
# (Bazzite / Steam Deck style) where a blank wait would look like a hang.
#
# Add this script as a non-Steam game. In its controller settings pick the
# "Gamepad" template so the browser sees a normal controller.
#
# Environment overrides: PORT (default 8765), PROGRESS_PORT (8766),
# BROWSER (command to run, defaults to the first browser found), NO_PULL=1.

set -uo pipefail
cd "$(dirname "$(readlink -f "$0")")"
export PATH="$HOME/.cargo/bin:$PATH"
export CARGO_TERM_COLOR=never CARGO_TERM_PROGRESS_WHEN=never
PORT="${PORT:-8765}"
PROGRESS_PORT="${PROGRESS_PORT:-8766}"
LOG="$PWD/build.log"
: > "$LOG"

log() { printf '[%s] %s\n' "$(date +%H:%M:%S)" "$*" >> "$LOG"; }

cleanup() {
  [ -n "${SERVE_PID:-}" ] && kill "$SERVE_PID" 2>/dev/null
  [ -n "${PROG_PID:-}" ] && kill "$PROG_PID" 2>/dev/null
}
trap cleanup EXIT

# 1. Progress page first, so something is on the TV within a second.
python3 tools/progress_server.py "$LOG" "$PROGRESS_PORT" "http://127.0.0.1:$PORT/" &
PROG_PID=$!
sleep 0.5

open_browser() {
  local url="$1"
  if [ -n "${BROWSER:-}" ]; then $BROWSER "$url" & return; fi
  if command -v flatpak >/dev/null 2>&1; then
    for app in org.mozilla.firefox com.brave.Browser com.google.Chrome org.chromium.Chromium com.microsoft.Edge; do
      if flatpak info "$app" >/dev/null 2>&1; then
        case "$app" in
          org.mozilla.firefox) flatpak run "$app" --kiosk "$url" & ;;
          *) flatpak run "$app" --kiosk --no-first-run "$url" & ;;
        esac
        return
      fi
    done
  fi
  for cmd in firefox chromium chromium-browser google-chrome brave-browser; do
    if command -v "$cmd" >/dev/null 2>&1; then
      case "$cmd" in
        firefox) "$cmd" --kiosk "$url" & ;;
        *) "$cmd" --kiosk --no-first-run "$url" & ;;
      esac
      return
    fi
  done
  xdg-open "$url" &
}
open_browser "http://127.0.0.1:$PROGRESS_PORT/"
BROWSER_PID=$!

# 2. Update and build, everything into the log the page is tailing.
build_ok=1
{
  if [ "${NO_PULL:-0}" != "1" ] && [ -d .git ]; then
    log "Pulling latest commits"
    git pull --ff-only 2>&1 || log "git pull failed, building what is here"
  fi
  if ! command -v cargo >/dev/null 2>&1; then
    log "Installing the Rust toolchain (first run only)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal 2>&1
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
  fi
  log "Checking wasm target"
  rustup target add wasm32-unknown-unknown 2>&1
  if ! command -v wasm-bindgen >/dev/null 2>&1; then
    log "Installing wasm-bindgen (first run only, a few minutes)"
    cargo install wasm-bindgen-cli --version 0.2.130 --locked 2>&1
  fi
  log "Building the simulation for the browser"
  cargo build --release --target wasm32-unknown-unknown -p evo_wasm 2>&1 || exit 1
  wasm-bindgen --target web --out-dir web/pkg --no-typescript target/wasm32-unknown-unknown/release/evo_wasm.wasm 2>&1 || exit 1
  log "Building the local server"
  cargo build --release -p evo_core --bin evo_serve 2>&1 || exit 1
  log "BUILD DONE"
} >> "$LOG" 2>&1 || build_ok=0

if [ "$build_ok" != "1" ]; then
  log "BUILD FAILED"
fi

# 3. Run the server (new build, or the previous one if the build failed).
BIN="target/release/evo_serve"
if [ ! -x "$BIN" ] && [ -x "dist/evo_serve" ]; then BIN="dist/evo_serve"; fi
if [ ! -x "$BIN" ]; then
  log "No evo_serve binary to run."
  wait "$BROWSER_PID"
  exit 1
fi
pkill -f "evo_serve --no-open --port $PORT" 2>/dev/null
"$BIN" --no-open --port "$PORT" &
SERVE_PID=$!
sleep 1
log "SERVER READY"

# 4. When the browser closes, stop everything (trap handles the kill).
wait "$BROWSER_PID"
