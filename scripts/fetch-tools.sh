#!/bin/sh
# Downloads yt-dlp and deno into vendor/, where the app looks for them, and
# checks each against the checksum its release publishes. The core resolves
# streams with yt-dlp, and yt-dlp needs deno to run YouTube's player code.
#
#   scripts/fetch-tools.sh            # only what is missing
#   scripts/fetch-tools.sh --refresh  # download again
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
vendor="$root/vendor"
refresh=${1:-}

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) ytdlp_name=yt-dlp_linux; deno_name=deno-x86_64-unknown-linux-gnu.zip ;;
    Linux-aarch64) ytdlp_name=yt-dlp_linux_aarch64; deno_name=deno-aarch64-unknown-linux-gnu.zip ;;
    Darwin-arm64) ytdlp_name=yt-dlp_macos; deno_name=deno-aarch64-apple-darwin.zip ;;
    Darwin-x86_64) ytdlp_name=yt-dlp_macos; deno_name=deno-x86_64-apple-darwin.zip ;;
    *) echo "no tools are published for $(uname -s) $(uname -m)" >&2; exit 1 ;;
esac

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

# Fails unless the file is what its release says it is.
verify() {
    actual=$(sha256 "$1")
    if [ "$actual" != "$2" ]; then
        rm -f "$1"
        echo "$1 does not match its published checksum" >&2
        exit 1
    fi
}

ytdlp="$vendor/yt-dlp"
if [ "$refresh" = --refresh ] || [ ! -x "$ytdlp/$ytdlp_name" ]; then
    [ "$refresh" != --refresh ] || rm -rf "$ytdlp"
    mkdir -p "$ytdlp"
    base=https://github.com/yt-dlp/yt-dlp/releases/latest/download
    # The unpacked build: the single-file one unpacks itself on every run,
    # which adds seconds to each stream resolve.
    zip="$vendor/$ytdlp_name.zip"
    curl -fsSL "$base/$ytdlp_name.zip" -o "$zip"
    expected=$(curl -fsSL "$base/SHA2-256SUMS" | awk -v name="$ytdlp_name.zip" '$2 == name { print $1 }')
    [ -n "$expected" ] || { echo "no checksum published for $ytdlp_name.zip" >&2; exit 1; }
    verify "$zip" "$expected"
    unzip -qo "$zip" -d "$ytdlp"
    chmod +x "$ytdlp/$ytdlp_name"
    # Which release this is, for the app's daily look for a newer one: it
    # downloads only when the release's archive differs from this.
    printf %s "$expected" > "$ytdlp/release.sha256"
    rm -f "$zip"
    echo 'yt-dlp ready'
fi

deno="$vendor/deno"
if [ "$refresh" = --refresh ] || [ ! -x "$deno/deno" ]; then
    [ "$refresh" != --refresh ] || rm -rf "$deno"
    mkdir -p "$deno"
    base=https://github.com/denoland/deno/releases/latest/download
    zip="$vendor/$deno_name"
    curl -fsSL "$base/$deno_name" -o "$zip"
    # Deno's checksum file holds the hash for this one download.
    expected=$(curl -fsSL "$base/$deno_name.sha256sum" | grep -oE '[0-9a-fA-F]{64}' | head -n1 | tr 'A-F' 'a-f')
    [ -n "$expected" ] || { echo "no checksum published for $deno_name" >&2; exit 1; }
    verify "$zip" "$expected"
    unzip -qo "$zip" -d "$deno"
    chmod +x "$deno/deno"
    rm -f "$zip"
    echo 'deno ready'
fi
