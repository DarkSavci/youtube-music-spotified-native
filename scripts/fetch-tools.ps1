# Downloads yt-dlp and deno into vendor/, where the app looks for them, and
# checks each against the checksum its release publishes. The core resolves
# streams with yt-dlp, and yt-dlp needs deno to run YouTube's player code.
#
#   scripts/fetch-tools.ps1            # only what is missing
#   scripts/fetch-tools.ps1 -Refresh   # download again
param([switch]$Refresh)

$ErrorActionPreference = 'Stop'
$vendor = Join-Path (Split-Path $PSScriptRoot -Parent) 'vendor'

function Get-Verified($url, $sumsUrl, $name, $destination) {
    Invoke-WebRequest $url -OutFile $destination
    $sums = (Invoke-WebRequest $sumsUrl).Content
    if ($sums -is [byte[]]) { $sums = [Text.Encoding]::UTF8.GetString($sums) }
    # yt-dlp publishes a list, one "hash  name" per line. deno publishes a
    # file for the one download, holding its hash in PowerShell's own layout.
    $hex = '[0-9a-fA-F]{64}'
    $named = $sums -split "`n" | Where-Object { $_ -match [regex]::Escape($name) -and $_ -match "^($hex)\s" }
    $all = [regex]::Matches($sums, $hex) | ForEach-Object { $_.Value.ToLower() } | Select-Object -Unique
    $expected = if ($named) {
        (($named | Select-Object -First 1) -split '\s+')[0].ToLower()
    } elseif (@($all).Count -eq 1) {
        @($all)[0]
    } else {
        throw "no checksum published for $name"
    }
    $actual = (Get-FileHash $destination -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $expected) {
        Remove-Item $destination
        throw "$name does not match its published checksum"
    }
}

$ytdlp = Join-Path $vendor 'yt-dlp'
if ($Refresh -or -not (Test-Path "$ytdlp\yt-dlp.exe")) {
    Remove-Item $ytdlp -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $ytdlp | Out-Null
    $base = 'https://github.com/yt-dlp/yt-dlp/releases/latest/download'
    # The unpacked build: the single-file one unpacks itself on every run,
    # which adds seconds to each stream resolve.
    $zip = Join-Path $vendor 'yt-dlp_win.zip'
    Get-Verified "$base/yt-dlp_win.zip" "$base/SHA2-256SUMS" 'yt-dlp_win.zip' $zip
    Expand-Archive $zip -DestinationPath $ytdlp -Force
    Remove-Item $zip
    Write-Host 'yt-dlp ready'
}

$deno = Join-Path $vendor 'deno'
if ($Refresh -or -not (Test-Path "$deno\deno.exe")) {
    Remove-Item $deno -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $deno | Out-Null
    $base = 'https://github.com/denoland/deno/releases/latest/download'
    $name = 'deno-x86_64-pc-windows-msvc.zip'
    $zip = Join-Path $vendor $name
    Get-Verified "$base/$name" "$base/$name.sha256sum" $name $zip
    Expand-Archive $zip -DestinationPath $deno -Force
    Remove-Item $zip
    Write-Host 'deno ready'
}
