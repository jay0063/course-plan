#!/usr/bin/env bash
# Builds a Debian/Ubuntu .deb (and a portable .tar.gz) for the host architecture.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
APP_NAME="course-plan"
VERSION="${VERSION:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)}"
MAINTAINER="${MAINTAINER:-CoursePlan Maintainer <maintainer@example.com>}"
DEB_ROOT="$ROOT/dist/deb-root"

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64)         DEB_ARCH=amd64 ;;
  aarch64|arm64)  DEB_ARCH=arm64 ;;
  *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

BIN="$ROOT/target/release/$APP_NAME"
[ -x "$BIN" ] || { echo "Build first: cargo build --release" >&2; exit 1; }

# Derive runtime library dependencies from the binary we actually built.
DEPS="libc6, libgcc-s1"
if command -v dpkg-shlibdeps >/dev/null 2>&1; then
  if DERIVED="$(dpkg-shlibdeps -Otarget "$BIN" 2>/dev/null)"; then
    DEPS="${DERIVED#shlibs:Depends=}"
  fi
fi
# Vulkan loader + the graphics stack GPUI links against at runtime.
DEPS="$DEPS, libvulkan1, libxkbcommon0, libx11-6, libwayland-client0, libfontconfig1, libfreetype6"

rm -rf "$DEB_ROOT"
mkdir -p "$DEB_ROOT/DEBIAN" \
         "$DEB_ROOT/usr/lib/$APP_NAME" \
         "$DEB_ROOT/usr/bin" \
         "$DEB_ROOT/usr/share/applications" \
         "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps" \
         "$DEB_ROOT/usr/share/metainfo" \
         "$DEB_ROOT/usr/share/doc/$APP_NAME"

install -m 0755 "$BIN" "$DEB_ROOT/usr/lib/$APP_NAME/$APP_NAME"
ln -sf "../lib/$APP_NAME/$APP_NAME" "$DEB_ROOT/usr/bin/$APP_NAME"
install -m 0644 "$ROOT/packaging/linux/course-plan.desktop" \
                "$DEB_ROOT/usr/share/applications/$APP_NAME.desktop"
install -m 0644 "$ROOT/packaging/icons/icon-256.png" \
                "$DEB_ROOT/usr/share/icons/hicolor/256x256/apps/$APP_NAME.png"
install -m 0644 "$ROOT/packaging/linux/$APP_NAME.metainfo.xml" \
                "$DEB_ROOT/usr/share/metainfo/$APP_NAME.metainfo.xml"
install -m 0644 "$ROOT/README.md" "$DEB_ROOT/usr/share/doc/$APP_NAME/README.md" 2>/dev/null || true

INSTALLED_SIZE="$(du -sk "$DEB_ROOT" | cut -f1)"

cat > "$DEB_ROOT/DEBIAN/control" <<EOF
Package: $APP_NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $DEB_ARCH
Depends: $DEPS
Installed-Size: $INSTALLED_SIZE
Maintainer: $MAINTAINER
Description: 小学课程管理计划
 统一管理学校课程与补习课程，生成每周计划表，支持日程提醒、
 备忘笔记与每日打卡。数据保存在本地，不联网。
EOF

dpkg-deb --root-owner-group --build "$DEB_ROOT" \
    "$ROOT/dist/${APP_NAME}_${VERSION}_${DEB_ARCH}.deb"

# Portable tarball for machines without a package manager.
PORTABLE="$ROOT/dist/portable/$APP_NAME"
rm -rf "$ROOT/dist/portable"
mkdir -p "$PORTABLE/bin" \
         "$PORTABLE/share/icons/hicolor/256x256/apps" \
         "$PORTABLE/share/applications"
install -m 0755 "$BIN" "$PORTABLE/bin/$APP_NAME"
install -m 0644 "$ROOT/packaging/icons/icon-256.png" \
    "$PORTABLE/share/icons/hicolor/256x256/apps/$APP_NAME.png"
install -m 0644 "$ROOT/packaging/linux/course-plan.desktop" \
    "$PORTABLE/share/applications/$APP_NAME.desktop"
tar -czf "$ROOT/dist/${APP_NAME}-${VERSION}-${DEB_ARCH}.tar.gz" \
    -C "$ROOT/dist/portable" "$APP_NAME"

echo "Built:"
echo "  dist/${APP_NAME}_${VERSION}_${DEB_ARCH}.deb"
echo "  dist/${APP_NAME}-${VERSION}-${DEB_ARCH}.tar.gz"
