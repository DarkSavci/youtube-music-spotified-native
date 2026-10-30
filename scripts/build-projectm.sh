#!/bin/sh
# Builds libprojectM, which draws MilkDrop, as a library the app loads when
# MilkDrop's window is opened: target/projectm/src/libprojectM/libprojectM-4.so
# or .dylib. Its source is fetched into vendor/projectm at the revision named
# here, the same one scripts/build-projectm.ps1 builds. It needs git, CMake,
# Ninja, a C++ compiler and, on Linux, OpenGL's headers.
#
#   scripts/build-projectm.sh            # only if it is not built yet
#   scripts/build-projectm.sh --rebuild  # fetch and build again
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
revision=98101f56feea576f8e240061c457541abc797aa1
source="$root/vendor/projectm/src"
build="${CARGO_TARGET_DIR:-$root/target}/projectm"
case "$(uname -s)" in
    Darwin) library="$build/src/libprojectM/libprojectM-4.dylib" ;;
    *) library="$build/src/libprojectM/libprojectM-4.so" ;;
esac

if [ -e "$library" ] && [ "${1:-}" != --rebuild ]; then
    echo "libprojectM is built: $library"
    exit 0
fi
if [ "${1:-}" = --rebuild ]; then
    rm -rf "$source" "$build"
fi
if [ ! -f "$source/CMakeLists.txt" ]; then
    rm -rf "$source"
    git clone --quiet https://github.com/projectM-visualizer/projectm.git "$source"
fi
git -C "$source" checkout --quiet "$revision"
git -C "$source" submodule update --init --recursive --quiet

# The playlist library is the app's own business and is left out.
cmake -S "$source" -B "$build" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DBUILD_SHARED_LIBS=ON \
    -DENABLE_PLAYLIST=OFF \
    -DENABLE_SYSTEM_PROJECTM_EVAL=OFF \
    -DBUILD_TESTING=OFF
cmake --build "$build"
[ -e "$library" ] || { echo "libprojectM was built, but not to $library" >&2; exit 1; }
echo "built $library"
