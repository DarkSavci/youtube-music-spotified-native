#!/bin/sh
# Builds the Go core to target/core, where a development build of the app
# looks for it.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target=${CARGO_TARGET_DIR:-$root/target}
cd "$root/core"
CGO_ENABLED=0 go build -o "$target/core/spotified-core" ./cmd/spotifier
