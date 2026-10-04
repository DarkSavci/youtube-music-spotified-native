# Builds the Go core to target/core, where a development build of the app
# looks for it.
$root = Split-Path $PSScriptRoot -Parent
Push-Location "$root\core"
try {
    $env:CGO_ENABLED = '0'
    go build -o "$root\target\core\spotified-core.exe" ./cmd/spotifier
    if ($LASTEXITCODE -ne 0) { throw 'go build failed' }
} finally {
    Pop-Location
}
