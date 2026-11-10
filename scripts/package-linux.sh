#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

arch=$(uname -m)
case "$arch" in
    x86_64|aarch64) ;;
    *) echo "Unsupported Linux architecture: $arch" >&2; exit 1 ;;
esac
target="$arch-unknown-linux-gnu"
platform="linux-$arch"
uv run --python 3.12 --with-requirements scripts/build-requirements.txt python scripts/fetch-tools.py
cargo build --locked --release --target "$target" --target-dir target

version=$(awk -F '"' '/^version = / { print $2; exit }' Cargo.toml)
name="ESP32-SuperFlash-$version-$platform"
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT HUP INT TERM
mkdir -p "$stage/$name/tools" dist
install -m 755 "target/$target/release/esp32-superflash" "$stage/$name/ESP32-SuperFlash"
cp -R "tools/$platform" tools/shared "$stage/$name/tools/"
cp README.md README.zh-CN.md LICENSE "$stage/$name/"
mkdir -p "$stage/$name/assets"
cp -R assets/screenshots "$stage/$name/assets/"
cp tools/README.md "$stage/$name/tools/"
tar -C "$stage" -czf "dist/$name.tar.gz" "$name"
printf '\n%s\n' "dist/$name.tar.gz"
