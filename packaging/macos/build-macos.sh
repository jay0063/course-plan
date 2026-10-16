#!/usr/bin/env bash
# Builds the macOS .app bundle and a DMG on a macOS machine.
#
#   ./packaging/macos/build-macos.sh                 # native architecture
#   ARCH=aarch64 ./packaging/macos/build-macos.sh     # Apple silicon
#   ARCH=x86_64  ./packaging/macos/build-macos.sh     # Intel
#
# Produces:
#   dist/课程计划.app
#   dist/课程计划-<version>-macos-<arch>.dmg
#   dist/课程计划-<version>-macos-<arch>.app.tar.gz   (notarization upload)
#
# Signing and notarization are opt-in via environment variables; see README.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN_NAME="course-plan"
APP_NAME="课程计划"
BUNDLE_ID="com.courseplan.app"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
ARCH="${ARCH:-}"
DIST="$ROOT/dist"
BUNDLE="$DIST/$APP_NAME.app"

if [ "$(uname -s)" != "Darwin" ]; then
    echo "This script must run on macOS." >&2
    exit 1
fi

if [ -z "$ARCH" ]; then
    case "$(uname -m)" in
        arm64) ARCH=aarch64 ;;
        *)     ARCH=x86_64 ;;
    esac
fi

case "$ARCH" in
    aarch64) RUST_TARGET=aarch64-apple-darwin ;;
    x86_64)  RUST_TARGET=x86_64-apple-darwin ;;
    *) echo "Unsupported ARCH: $ARCH" >&2; exit 1 ;;
esac

echo "==> Building $RUST_TARGET"
rustup target add "$RUST_TARGET"
cargo build --release --locked --target "$RUST_TARGET"

BIN="$ROOT/target/$RUST_TARGET/release/$BIN_NAME"
[ -x "$BIN" ] || { echo "Binary not found at $BIN" >&2; exit 1; }

echo "==> Assembling $BUNDLE"
rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS" "$BUNDLE/Contents/Resources"
install -m 0755 "$BIN" "$BUNDLE/Contents/MacOS/$BIN_NAME"
install -m 0644 "$ROOT/packaging/icons/course-plan.icns" "$BUNDLE/Contents/course-plan.icns"

# Keep one version source of truth between Info.plist and Cargo.toml.
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$BUNDLE/Contents/Info.plist" \
  || cp "$ROOT/packaging/macos/Info.plist" "$BUNDLE/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleExecutable $BIN_NAME" "$BUNDLE/Contents/Info.plist"

printf 'APPL????' > "$BUNDLE/Contents/PkgInfo"

echo "==> Signing"
if [ -n "${SIGN_IDENTITY:-}" ]; then
    # Sign nested code first, then the bundle, from the inside out.
    codesign --force --timestamp --options runtime \
        --sign "$SIGN_IDENTITY" "$BUNDLE/Contents/MacOS/$BIN_NAME"
    codesign --force --timestamp --options runtime \
        --sign "$SIGN_IDENTITY" "$BUNDLE"
    codesign --verify --deep --strict --verbose=2 "$BUNDLE"
else
    # Ad hoc signature so the bundle runs locally. Not valid for distribution.
    codesign --force --deep --sign - "$BUNDLE"
    echo "    ad-hoc signature only (set SIGN_IDENTITY for a real release)"
fi

DMG="$DIST/$APP_NAME-$VERSION-macos-$ARCH.dmg"
TARBALL="$DIST/$APP_NAME-$VERSION-macos-$ARCH.app.tar.gz"

echo "==> Creating DMG"
rm -f "$DMG"
STAGE="$(mktemp -d)"
cp -R "$BUNDLE" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"

echo "==> Creating notarization tarball"
tar -czf "$TARBALL" -C "$DIST" "$APP_NAME.app"

if [ -n "${SIGN_IDENTITY:-}" ]; then
    codesign --force --timestamp --sign "$SIGN_IDENTITY" "$DMG"
    if [ -n "${NOTARY_PROFILE:-}" ]; then
        xcrun notarytool submit "$DMG" --keychain-profile "$NOTARY_PROFILE" --wait
        xcrun stapler staple "$DMG"
        xcrun stapler validate "$DMG"
    fi
fi

echo "==> Done"
echo "    $BUNDLE"
echo "    $DMG"
echo "    $TARBALL"
