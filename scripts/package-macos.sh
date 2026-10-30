#!/bin/sh
# Builds everything and gathers it into dist/ as an app: a .dmg to install it
# from, and a .tar.gz of it for the updater. Inside are the app, the Go core,
# yt-dlp and deno under vendor/, and libprojectM, which draws MilkDrop, with
# its licence.
#
# It is signed ad hoc, which needs no account and is what Apple Silicon asks
# of anything it runs. It is not notarised, so the first launch has to be
# approved in System Settings, under Privacy & Security.
#
#   scripts/package-macos.sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target=${CARGO_TARGET_DIR:-$root/target}
cd "$root"

sh scripts/build-core.sh
sh scripts/fetch-tools.sh
sh scripts/build-projectm.sh
cargo build --release -p spotified

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
case "$(uname -m)" in
    arm64) arch=arm64 ;;
    *) arch=x64 ;;
esac
base="youtube-music-spotified-native-$version-macos-$arch"
name="Youtube Music Spotified"
app="$root/dist/$name.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources/vendor"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
cp packaging/macos/icon.icns "$app/Contents/Resources/icon.icns"
cp "$target/release/spotified" "$app/Contents/MacOS/spotified"
resources="$app/Contents/Resources"
cp "$target/core/spotified-core" "$resources/spotified-core"
cp -R vendor/yt-dlp "$resources/vendor/yt-dlp"
cp -R vendor/deno "$resources/vendor/deno"
cp LICENSE NOTICE.md "$resources/"
# A library of its own beside the app, under its own licence (LGPL), so that
# it can be replaced with another build of itself.
cp -L "$target/projectm/src/libprojectM/libprojectM-4.dylib" "$resources/"
cp vendor/projectm/src/LICENSE.txt "$resources/libprojectM-LICENSE.txt"

# Each thing that runs by itself, and then the whole.
codesign --force --sign - "$resources/spotified-core" "$resources/vendor/deno/deno" \
    "$resources/libprojectM-4.dylib"
codesign --force --sign - "$app/Contents/MacOS/spotified"
codesign --force --sign - "$app"

tar -czf "$root/dist/$base.tar.gz" -C "$root/dist" "$name.app"
echo "packaged $root/dist/$base.tar.gz"

staging=$(mktemp -d)
cp -R "$app" "$staging/"
ln -s /Applications "$staging/Applications"
rm -f "$root/dist/$base.dmg"
hdiutil create -quiet -volname "$name" -srcfolder "$staging" -ov -format UDZO "$root/dist/$base.dmg"
rm -rf "$staging"
echo "packaged $root/dist/$base.dmg"
