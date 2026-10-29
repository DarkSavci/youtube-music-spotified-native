# Builds the Go core into an Android library, android/app/libs/core.aar,
# for phones (arm64) and the emulator (x86_64). Needs the Android SDK with
# its NDK, and gomobile (go install golang.org/x/mobile/cmd/gomobile@latest).
$root = Split-Path $PSScriptRoot -Parent
$sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { "$env:LOCALAPPDATA\Android\Sdk" }
$ndk = Get-ChildItem "$sdk\ndk" -Directory -ErrorAction SilentlyContinue |
    Sort-Object { [version]$_.Name } | Select-Object -Last 1
if (-not $ndk) { throw "no NDK under $sdk\ndk" }
$env:ANDROID_HOME = $sdk
$env:ANDROID_NDK_HOME = $ndk.FullName

# gomobile runs gobind, and finds it only on PATH.
$env:PATH = "$env:PATH;$(go env GOPATH)\bin"
$gomobile = Get-Command gomobile

New-Item -ItemType Directory -Force "$root\android\app\libs" | Out-Null
Push-Location "$root\core"
try {
    & $gomobile bind -target 'android/arm64,android/amd64' -androidapi 26 `
        -javapkg dev.darksavci.spotified.core -trimpath -ldflags '-s -w' `
        -o "$root\android\app\libs\core.aar" ./mobile
    if ($LASTEXITCODE -ne 0) { throw 'gomobile bind failed' }
} finally {
    Pop-Location
}
