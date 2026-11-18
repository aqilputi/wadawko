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
TARGET_DIR=${CARGO_TARGET_DIR:-target}
REL="$TARGET_DIR/release"
NAME="wadawko-$VERSION-linux-x86_64"
OUT="dist/$NAME"

for b in wadawko wadawko-cli wadawko-render; do
  [ -x "$REL/$b" ] || { echo "falta $REL/$b — rode: cargo build --release -p wadawko-app -p wadawko-cli -p wadawko-export" >&2; exit 1; }
done

APP_ID=io.github.aqilputi.WaDAWko
rm -rf "$OUT" && mkdir -p "$OUT/bin" "$OUT/share/applications" "$OUT/share/metainfo" "$OUT/share/icons" "$OUT/licenses"
install -m755 "$REL/wadawko" "$REL/wadawko-cli" "$REL/wadawko-render" "$OUT/bin/"
strip "$OUT"/bin/* 2>/dev/null || true
install -m644 "assets/linux/$APP_ID.desktop" "$OUT/share/applications/"
install -m644 "assets/linux/$APP_ID.metainfo.xml" "$OUT/share/metainfo/"
cp -r assets/linux/icons/hicolor "$OUT/share/icons/"
install -m644 README.md CHANGELOG.md LICENSE-MIT LICENSE-APACHE NOTICE "$OUT/"
install -m644 assets/samples/CREDITS.md "$OUT/licenses/SAMPLES-CREDITS.md"
install -m644 assets/fonts/OFL-*.txt assets/fonts/FONTLOG-Bravura.txt "$OUT/licenses/"

cat > "$OUT/install.sh" <<'EOF'
#!/usr/bin/env bash
# Instala o WaDAWko em ~/.local (binários em ~/.local/bin, atalho no menu de aplicativos).
set -euo pipefail
cd "$(dirname "$0")"
PREFIX=${PREFIX:-$HOME/.local}
APP_ID=io.github.aqilputi.WaDAWko
mkdir -p "$PREFIX/bin" "$PREFIX/share/applications" "$PREFIX/share/metainfo" "$PREFIX/share/icons"
install -m755 bin/wadawko bin/wadawko-cli bin/wadawko-render "$PREFIX/bin/"
sed "s|^Exec=wadawko|Exec=$PREFIX/bin/wadawko|" "share/applications/$APP_ID.desktop" > "$PREFIX/share/applications/$APP_ID.desktop"
rm -f "$PREFIX/share/applications/wadawko.desktop"   # nome antigo (até a 0.1.0)
install -m644 "share/metainfo/$APP_ID.metainfo.xml" "$PREFIX/share/metainfo/"
cp -r share/icons/hicolor "$PREFIX/share/icons/"
command -v update-desktop-database >/dev/null && update-desktop-database "$PREFIX/share/applications" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true
echo "WaDAWko instalado em $PREFIX/bin."
echo "Músicas do app web ou de outro PC: Config → importar músicas."
EOF
chmod +x "$OUT/install.sh"

tar -C dist -czf "dist/$NAME.tar.gz" "$NAME"
(cd dist && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
echo "dist/$NAME.tar.gz ($(du -h "dist/$NAME.tar.gz" | cut -f1))"
