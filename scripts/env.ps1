# Puts the Rust toolchain and MinGW (gcc, dlltool, windres) on PATH for this
# shell. This machine builds with the GNU toolchain because the MSVC build
# tools are not installed. Dot-source it: `. scripts/env.ps1`.
$mingw = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Directory -Filter 'BrechtSanders.WinLibs*' |
    Select-Object -First 1
if (-not $mingw) { throw 'WinLibs MinGW is not installed: winget install BrechtSanders.WinLibs.POSIX.UCRT' }
$env:PATH = "$env:USERPROFILE\.cargo\bin;$($mingw.FullName)\mingw64\bin;$env:PATH"
