#!/bin/sh
# Builds everything and gathers it into dist/: a folder that runs from
# anywhere, a .tar.gz of it, and an AppImage, which is the copy that updates
# itself: the app, the Go core beside it, yt-dlp and deno under vendor/, and
# libprojectM, which draws MilkDrop, with its licence.
#
#   scripts/package-linux.sh
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target=${CARGO_TARGET_DIR:-$root/target}
cd "$root"

sh scripts/build-core.sh
sh scripts/fetch-tools.sh
sh scripts/build-projectm.sh
cargo build --release -p spotified

version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
arch=$(uname -m)
base="youtube-music-spotified-native-$version-linux-$arch"
folder="$root/dist/Youtube Music Spotified"
rm -rf "$folder"
mkdir -p "$folder/vendor"
cp "$target/release/spotified" "$folder/spotified"
cp "$target/core/spotified-core" "$folder/spotified-core"
cp -R vendor/yt-dlp "$folder/vendor/yt-dlp"
cp -R vendor/deno "$folder/vendor/deno"
cp LICENSE NOTICE.md "$folder/"
# A library of its own beside the app, under its own licence (LGPL), so that
# it can be replaced with another build of itself.
cp -L "$target/projectm/src/libprojectM/libprojectM-4.so" "$folder/"
cp vendor/projectm/src/LICENSE.txt "$folder/libprojectM-LICENSE.txt"

tar -czf "$root/dist/$base.tar.gz" -C "$root/dist" "Youtube Music Spotified"
echo "packaged $root/dist/$base.tar.gz"

# The AppImage is the same folder with what a desktop needs to show it.
tools="$target/appimage"
mkdir -p "$tools"
tool="$tools/appimagetool-$arch.AppImage"
if [ ! -x "$tool" ]; then
    curl -fsSL "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$arch.AppImage" -o "$tool"
    chmod +x "$tool"
fi
appdir="$tools/AppDir"
rm -rf "$appdir"
cp -R "$folder" "$appdir"
cp packaging/linux/AppRun "$appdir/AppRun"
chmod +x "$appdir/AppRun"
cp packaging/linux/dev.darksavci.spotified.native.desktop "$appdir/"
cp packaging/linux/icon.png "$appdir/dev.darksavci.spotified.native.png"
# Without FUSE, as on a build machine in a container.
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=$arch "$tool" --no-appstream "$appdir" "$root/dist/$base.AppImage"
echo "packaged $root/dist/$base.AppImage"
