#!/usr/bin/env bash
# Gera o AppImage (Linux x86_64) a partir dos binários release, com linuxdeploy.
#
# uso: tools/appimage.sh [versão]        (padrão: versão do workspace no Cargo.toml)
# requer: cargo build --release -p wadawko-app -p wadawko-cli -p wadawko-export
# saída:  dist/WaDAWko-<versão>-x86_64.AppImage (+ .sha256)
#
# O linuxdeploy é baixado para tools/.cache/ na primeira vez. O AppImage leva as
# bibliotecas do sistema de build (xkbcommon, xcb, alsa, freetype…), menos as que
# devem vir da máquina de quem roda (glibc, libGL/libvulkan e drivers): compile na
# distro mais antiga que quiser suportar (a CI usa Ubuntu 24.04).
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}
VERSION=${VERSION#v}
REL="${CARGO_TARGET_DIR:-target}/release"
APP_ID=io.github.aqilputi.WaDAWko
APPDIR="dist/AppDir"

for b in wadawko wadawko-cli wadawko-render; do
  [ -x "$REL/$b" ] || { echo "falta $REL/$b — rode: cargo build --release -p wadawko-app -p wadawko-cli -p wadawko-export" >&2; exit 1; }
done

LD=tools/.cache/linuxdeploy-x86_64.AppImage
if [ ! -x "$LD" ]; then
  mkdir -p tools/.cache
  curl -fsSL -o "$LD" https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
  chmod +x "$LD"
fi

rm -rf "$APPDIR" && mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/metainfo" "$APPDIR/usr/share/icons" "$APPDIR/usr/share/doc/wadawko"
install -m755 "$REL/wadawko" "$REL/wadawko-cli" "$REL/wadawko-render" "$APPDIR/usr/bin/"
strip "$APPDIR"/usr/bin/* 2>/dev/null || true
install -m644 "assets/linux/$APP_ID.metainfo.xml" "$APPDIR/usr/share/metainfo/"
cp -r assets/linux/icons/hicolor "$APPDIR/usr/share/icons/"
install -m644 README.md CHANGELOG.md LICENSE-MIT LICENSE-APACHE NOTICE assets/samples/CREDITS.md "$APPDIR/usr/share/doc/wadawko/"

# --appimage-extract-and-run: dispensa FUSE (CI, containers)
export ARCH=x86_64 LINUXDEPLOY_OUTPUT_VERSION="$VERSION" APPIMAGE_EXTRACT_AND_RUN=1
OUTPUT="dist/WaDAWko-$VERSION-x86_64.AppImage" "$LD" --appimage-extract-and-run \
  --appdir "$APPDIR" \
  --executable "$APPDIR/usr/bin/wadawko" \
  --desktop-file "assets/linux/$APP_ID.desktop" \
  --icon-file "assets/linux/icons/hicolor/256x256/apps/$APP_ID.png" \
  --output appimage
(cd dist && sha256sum "WaDAWko-$VERSION-x86_64.AppImage" > "WaDAWko-$VERSION-x86_64.AppImage.sha256")
rm -rf "$APPDIR"
echo "dist/WaDAWko-$VERSION-x86_64.AppImage ($(du -h "dist/WaDAWko-$VERSION-x86_64.AppImage" | cut -f1))"
