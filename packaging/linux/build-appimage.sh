#!/usr/bin/env bash
# Builds the AppDir layout that appimagetool turns into an AppImage.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP_NAME="course-plan"
VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)}"
ARCH="${ARCH:-$(uname -m)}"
STAGE="$ROOT/dist/appimage-root"
APPDIR="$STAGE/${APP_NAME}.AppDir"

case "$ARCH" in
  x86_64|amd64)   ARCH_NAME=x86_64 ;;
  aarch64|arm64)  ARCH_NAME=aarch64 ;;
  *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

BIN="$ROOT/target/release/$APP_NAME"
[ -x "$BIN" ] || { echo "Build first: cargo build --release" >&2; exit 1; }

rm -rf "$STAGE"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/256x256/apps" \
         "$APPDIR/usr/share/metainfo"

install -m 0755 "$BIN" "$APPDIR/usr/bin/$APP_NAME"
install -m 0644 "$ROOT/packaging/icons/icon-256.png" "$APPDIR/${APP_NAME}.png"
install -m 0644 "$ROOT/packaging/linux/course-plan.metainfo.xml" \
                "$APPDIR/usr/share/metainfo/${APP_NAME}.metainfo.xml"
install -m 0644 "$ROOT/packaging/linux/AppRun" "$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun"

# Top-level AppImage desktop entry: Exec names the AppImage's own binary.
cat > "$APPDIR/${APP_NAME}.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=课程计划
Name[en]=CoursePlan
Comment=小学课程管理计划
Exec=${APP_NAME}
Icon=${APP_NAME}
Terminal=false
Categories=Utility;Education;Office;
X-AppImage-Version=${VERSION}
X-AppImage-Arch=${ARCH_NAME}
EOF

install -m 0644 "$APPDIR/${APP_NAME}.desktop" \
                "$APPDIR/usr/share/applications/${APP_NAME}.desktop"

# appimagetool honours APPIMAGE and ICON environment variables.
OUT="$ROOT/dist/${APP_NAME}-${VERSION}-${ARCH_NAME}.AppImage"
APPIMAGE="$APPDIR" ICON="$APPDIR/${APP_NAME}.png" \
  appimagetool "$APPDIR" "$OUT"

echo "Built: $OUT"
