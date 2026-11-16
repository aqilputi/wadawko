#!/usr/bin/env bash
# Empacota os binários release num .tar.gz para Linux x86_64 (o mesmo localmente e na CI).
#
# uso: tools/package.sh [versão]        (padrão: versão do workspace no Cargo.toml)
# requer: cargo build --release -p wadawko-app -p wadawko-cli -p wadawko-export
# saída:  dist/wadawko-<versão>-linux-x86_64.tar.gz (+ .sha256)
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}
VERSION=${VERSION#v}
TARGET_DIR=${CARGO_TARGET_DIR:-$(sed -n 's/^target-dir = "\(.*\)"/\1/p' .cargo/config.toml 2>/dev/null || true)}
TARGET_DIR=${TARGET_DIR:-target}
REL="$TARGET_DIR/release"
NAME="wadawko-$VERSION-linux-x86_64"
OUT="dist/$NAME"

for b in wadawko wadawko-cli wadawko-render; do
  [ -x "$REL/$b" ] || { echo "falta $REL/$b — rode: cargo build --release -p wadawko-app -p wadawko-cli -p wadawko-export" >&2; exit 1; }
done

rm -rf "$OUT" && mkdir -p "$OUT/bin" "$OUT/share/applications" "$OUT/licenses"
install -m755 "$REL/wadawko" "$REL/wadawko-cli" "$REL/wadawko-render" "$OUT/bin/"
strip "$OUT"/bin/* 2>/dev/null || true
install -m644 assets/linux/wadawko.desktop "$OUT/share/applications/"
install -m644 README.md CHANGELOG.md "$OUT/"
install -m644 public/samples/CREDITS.md "$OUT/licenses/SAMPLES-CREDITS.md"
install -m644 assets/fonts/OFL-*.txt assets/fonts/FONTLOG-Bravura.txt "$OUT/licenses/"

cat > "$OUT/install.sh" <<'EOF'
#!/usr/bin/env bash
# Instala o WaDAWko em ~/.local (binários em ~/.local/bin, atalho no menu de aplicativos).
set -euo pipefail
cd "$(dirname "$0")"
PREFIX=${PREFIX:-$HOME/.local}
mkdir -p "$PREFIX/bin" "$PREFIX/share/applications"
install -m755 bin/wadawko bin/wadawko-cli bin/wadawko-render "$PREFIX/bin/"
sed "s|^Exec=wadawko|Exec=$PREFIX/bin/wadawko|" share/applications/wadawko.desktop > "$PREFIX/share/applications/wadawko.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
echo "WaDAWko instalado em $PREFIX/bin. Primeira vez vindo do app web:"
echo "  wadawko --migrate-from <caminho>/data/wadawko.db"
EOF
chmod +x "$OUT/install.sh"

tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
(cd dist && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
echo "dist/$NAME.tar.gz ($(du -h "dist/$NAME.tar.gz" | cut -f1))"
