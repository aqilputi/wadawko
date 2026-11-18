#!/usr/bin/env bash
# Roda o app num compositor SEM TELA (weston headless + Xwayland), para o probe,
# o selftest e os demos de medição não abrirem janela na tela de ninguém:
#
#   tools/headless.sh --db /tmp/teste.db --mute --demo probe | grep PROBE
#
# Precisa de `weston` (com xwayland). O X11 tem ponteiro/teclado virtuais, que o
# backend headless do weston não oferece em Wayland (o gpui exige wl_seat).
# Renderização em software (pixman): ~10 fps — serve para conferir comportamento
# e comparar medidas entre versões, não para fps absoluto.
set -euo pipefail
BIN="${WADAWKO_BIN:-$(dirname "$0")/../spike/target/release/wadawko}"
[ -x "$BIN" ] || BIN="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys;print(json.load(sys.stdin)["target_directory"])')/release/wadawko"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
SOCK="wl-wadawko-headless-$$"
LOG="$(mktemp)"
weston --backend=headless --renderer=pixman --xwayland --socket="$SOCK" --width=1600 --height=1000 --no-config >"$LOG" 2>&1 &
WPID=$!
trap 'kill $WPID 2>/dev/null; rm -f "$LOG"' EXIT
for _ in $(seq 50); do
    D=$(grep -o 'xserver listening on display :[0-9]*' "$LOG" | grep -o ':[0-9]*' || true)
    [ -n "$D" ] && break
    sleep 0.1
done
[ -n "${D:-}" ] || { echo "weston/Xwayland não subiu:" >&2; cat "$LOG" >&2; exit 2; }
# config em pasta temporária: o probe mexe no metrônomo/vim e regravaria o settings.json
CFG="$(mktemp -d)"
env -u WAYLAND_DISPLAY DISPLAY="$D" XDG_CONFIG_HOME="$CFG" "$BIN" "$@"
