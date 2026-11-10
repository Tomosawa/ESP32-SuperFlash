#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

arch=$(uname -m)
case "$arch" in
    arm64) arch=aarch64 ;;
    x86_64) ;;
    *) echo "Unsupported macOS architecture: $arch" >&2; exit 1 ;;
esac
target="$arch-apple-darwin"
platform="macos-$arch"
uv run --python 3.12 --with-requirements scripts/build-requirements.txt python scripts/fetch-tools.py
cargo build --locked --release --target "$target" --target-dir target

version=$(awk -F '"' '/^version = / { print $2; exit }' Cargo.toml)
name="ESP32-SuperFlash-$version-$platform"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT HUP INT TERM
app="$stage/ESP32-SuperFlash.app"
resources="$app/Contents/Resources"
mkdir -p "$app/Contents/MacOS" "$resources/tools" "$stage/SuperFlash.iconset" dist
install -m 755 "target/$target/release/esp32-superflash" "$app/Contents/MacOS/ESP32-SuperFlash"
cp -R "tools/$platform" tools/shared "$resources/tools/"
cp README.md README.zh-CN.md LICENSE "$resources/"
mkdir -p "$resources/assets"
cp -R assets/screenshots "$resources/assets/"
cp tools/README.md "$resources/tools/"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "https://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key><string>ESP32-SuperFlash</string>
    <key>CFBundleIdentifier</key><string>io.github.tomosawa.esp32-superflash</string>
    <key>CFBundleName</key><string>ESP32 SuperFlash</string>
    <key>CFBundleDisplayName</key><string>ESP32 SuperFlash</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>CFBundleIconFile</key><string>SuperFlash</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
    <key>LSMinimumSystemVersion</key><string>14.0</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
for size in 16 32 128 256 512; do
    sips -z "$size" "$size" assets/icon.png --out "$stage/SuperFlash.iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z "$double" "$double" assets/icon.png --out "$stage/SuperFlash.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$stage/SuperFlash.iconset" -o "$resources/SuperFlash.icns"

# Preserve valid upstream signatures and sign the remaining Mach-O files before sealing the app.
find "$resources/tools/$platform" -type f -exec sh -ec '
    for tool do
        case "$(file -b "$tool")" in
            Mach-O*)
                if ! codesign --verify --strict "$tool" 2>/dev/null; then
                    codesign --force --sign - "$tool"
                    codesign --verify --strict "$tool"
                fi
                ;;
        esac
    done
' sh {} +
codesign --force --sign - "$app"
codesign --verify --deep --strict --verbose=2 "$app"

mkdir "$stage/dmg"
mv "$app" "$stage/dmg/"
ln -s /Applications "$stage/dmg/Applications"
dmg="$stage/$name.dmg"
hdiutil create -format UDZO -fs HFS+ -volname "ESP32 SuperFlash" -srcfolder "$stage/dmg" "$dmg"
codesign --sign - --identifier io.github.tomosawa.esp32-superflash.dmg "$dmg"
codesign --verify --strict --verbose=2 "$dmg"
mv -f "$dmg" "dist/$name.dmg"
rm -rf "dist/ESP32-SuperFlash.app"
ditto "$stage/dmg/ESP32-SuperFlash.app" "dist/ESP32-SuperFlash.app"
printf '\n%s\n%s\n' "dist/ESP32-SuperFlash.app" "dist/$name.dmg"
