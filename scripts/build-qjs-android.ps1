# Builds QuickJS-NG's qjs for Android, the JavaScript runtime yt-dlp solves
# YouTube's player challenges with there, as deno does on the desktop.
#
# It is a program, not a library, but it goes in the app as
# jniLibs/<abi>/libqjs.so: the only place Android lets an app run a file
# from is its native library folder, and only files named like libraries
# are put there.
#
# The release's amalgamated source has the engine but not the qjs program,
# whose --help yt-dlp reads its version from and whose --script it runs, so
# this builds from the tagged source as the project's CMakeLists does.
$version = 'v0.17.0'
# GitHub's archive of the tag, as first fetched; it has no published sum.
$sha256 = '559bc4c420475e55c7ab4510adbc562f55d7524d75e8e89d79ce4bb02f5687d9'
$api = 26

$root = Split-Path $PSScriptRoot -Parent
$sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { "$env:LOCALAPPDATA\Android\Sdk" }
$ndk = Get-ChildItem "$sdk\ndk" -Directory -ErrorAction SilentlyContinue |
    Sort-Object { [version]$_.Name } | Select-Object -Last 1
if (-not $ndk) { throw "no NDK under $sdk\ndk" }
$bin = "$($ndk.FullName)\toolchains\llvm\prebuilt\windows-x86_64\bin"

$work = "$root\target\qjs-android"
New-Item -ItemType Directory -Force $work | Out-Null
$archive = "$work\quickjs-$version.tar.gz"
if (-not (Test-Path $archive)) {
    Invoke-WebRequest "https://github.com/quickjs-ng/quickjs/archive/refs/tags/$version.tar.gz" -OutFile $archive
}
if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne $sha256) {
    Remove-Item $archive
    throw "the quickjs $version source does not match its recorded checksum"
}
$src = "$work\quickjs-$version"
if (Test-Path $src) { Remove-Item -Recurse -Force $src }
New-Item -ItemType Directory $src | Out-Null
# Windows' own tar: Git's, if it comes first on PATH, reads "C:" as a host.
& "$env:SystemRoot\System32\tar.exe" -xzf $archive -C $src --strip-components=1
if ($LASTEXITCODE -ne 0) { throw 'could not unpack the quickjs source' }

$sources = 'dtoa.c', 'libregexp.c', 'libunicode.c', 'quickjs.c', 'quickjs-libc.c',
    'gen/repl.c', 'gen/standalone.c', 'qjs.c' | ForEach-Object { "$src\$_" }
$abis = @{ 'arm64-v8a' = 'aarch64-linux-android'; 'x86_64' = 'x86_64-linux-android' }
foreach ($abi in $abis.Keys) {
    $out = "$root\android\app\src\main\jniLibs\$abi"
    New-Item -ItemType Directory -Force $out | Out-Null
    & "$bin\clang.exe" "--target=$($abis[$abi])$api" -O2 -s -fPIE -pie -w `
        -DQUICKJS_NG_BUILD -D_GNU_SOURCE "-I$src" `
        @sources -lm -ldl -o "$out\libqjs.so"
    if ($LASTEXITCODE -ne 0) { throw "qjs for $abi failed" }
}
