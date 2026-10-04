# Builds everything and gathers it into dist/, as a folder that runs from
# anywhere and as a zip of it: the app, the Go core beside it, and yt-dlp
# and deno under vendor/, which is where the app looks for them.
#
#   scripts/package.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot\env.ps1"

Push-Location $root
try {
    & "$PSScriptRoot\build-core.ps1"
    & "$PSScriptRoot\fetch-tools.ps1"
    cargo build --release -p spotified
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

    $version = (Select-String -Path "$root\Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
    $name = 'Youtube Music Spotified'
    $folder = Join-Path $root "dist\$name"
    Remove-Item $folder -Recurse -Force -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force "$folder\vendor" | Out-Null

    Copy-Item "$root\target\release\spotified.exe" "$folder\$name.exe"
    Copy-Item "$root\target\core\spotified-core.exe" $folder
    Copy-Item "$root\vendor\yt-dlp" "$folder\vendor\yt-dlp" -Recurse
    Copy-Item "$root\vendor\deno" "$folder\vendor\deno" -Recurse
    Copy-Item "$root\LICENSE", "$root\NOTICE.md" $folder

    $zip = Join-Path $root "dist\youtube-music-spotified-native-$version-windows-x64.zip"
    Remove-Item $zip -ErrorAction SilentlyContinue
    Compress-Archive -Path $folder -DestinationPath $zip
    $size = [math]::Round((Get-ChildItem $folder -Recurse | Measure-Object Length -Sum).Sum / 1MB)
    Write-Host "packaged $folder ($size MB) and $zip"

    # The installer, where Inno Setup is installed to build it.
    $iscc = @("$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe", "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe") |
        Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($iscc) {
        & $iscc /Q "/DVersion=$version" "$root\packaging\windows\spotified.iss"
        if ($LASTEXITCODE -ne 0) { throw 'the installer did not compile' }
        Write-Host "installer: dist\youtube-music-spotified-native-$version-setup.exe"
        # What a release publishes beside its files; the app checks a
        # downloaded update against it.
        $sums = Get-ChildItem "$root\dist" -File -Include "*-$version-setup.exe", "*-$version-windows-x64.zip" -Name |
            ForEach-Object { "$((Get-FileHash "$root\dist\$_" -Algorithm SHA256).Hash.ToLower())  $_" }
        Set-Content "$root\dist\SHA256SUMS.txt" $sums -Encoding ascii
    } else {
        Write-Host 'Inno Setup is not installed; no installer built'
    }
} finally {
    Pop-Location
}
