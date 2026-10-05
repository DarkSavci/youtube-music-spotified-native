# Builds libprojectM, which draws MilkDrop, as a library the app loads when
# MilkDrop's window is opened: target/projectm/src/libprojectM/libprojectM-4.dll.
# Its source is fetched into vendor/projectm at the revision named here.
# It needs git, CMake, Ninja and a C++ compiler; scripts/env.ps1 puts
# MinGW's on PATH. The library stands alone: nothing of the compiler's is
# needed beside it.
#
#   scripts/build-projectm.ps1            # only if it is not built yet
#   scripts/build-projectm.ps1 -Rebuild   # fetch and build again
param([switch]$Rebuild)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot\env.ps1"

# What Spotifast's projectm-sys builds: 4.1 with the loader that finds
# OpenGL by itself, so nothing else has to be built beside it.
$revision = '98101f56feea576f8e240061c457541abc797aa1'
$source = Join-Path $root 'vendor\projectm\src'
$build = Join-Path $root 'target\projectm'
$library = Join-Path $build 'src\libprojectM\libprojectM-4.dll'

if ((Test-Path $library) -and -not $Rebuild) {
    Write-Host "libprojectM is built: $library"
    return
}

if ($Rebuild) {
    Remove-Item $source, $build -Recurse -Force -ErrorAction SilentlyContinue
}
if (-not (Test-Path "$source\CMakeLists.txt")) {
    Remove-Item $source -Recurse -Force -ErrorAction SilentlyContinue
    git clone --quiet https://github.com/projectM-visualizer/projectm.git $source
    if ($LASTEXITCODE -ne 0) { throw 'libprojectM could not be fetched' }
}
git -C $source checkout --quiet $revision
if ($LASTEXITCODE -ne 0) { throw "libprojectM has no revision $revision" }
git -C $source submodule update --init --recursive --quiet
if ($LASTEXITCODE -ne 0) { throw "libprojectM's own dependencies could not be fetched" }

# Static inside, so the one file is all there is to ship; the playlist
# library is the app's own business and is left out.
cmake -S $source -B $build -G Ninja `
    -DCMAKE_BUILD_TYPE=Release `
    -DBUILD_SHARED_LIBS=ON `
    -DENABLE_PLAYLIST=OFF `
    -DENABLE_SYSTEM_PROJECTM_EVAL=OFF `
    -DBUILD_TESTING=OFF `
    '-DCMAKE_SHARED_LINKER_FLAGS=-static -static-libgcc -static-libstdc++'
if ($LASTEXITCODE -ne 0) { throw 'libprojectM could not be configured' }
cmake --build $build
if ($LASTEXITCODE -ne 0) { throw 'libprojectM did not build' }
if (-not (Test-Path $library)) { throw "libprojectM was built, but not to $library" }
Write-Host "built $library"
