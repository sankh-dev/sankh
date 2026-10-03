#!/bin/sh
# Sankh installer: curl -fsSL https://sankh.dev/install.sh | sh
#
# Environment:
#   SANKH_VERSION      version to install, e.g. 0.2.0 (default: latest release)
#   SANKH_INSTALL_DIR  install directory (default: ~/.local/bin)
#
# Downloads the release archive for this platform from GitHub, verifies its
# sha256 checksum, and installs the `sankh` binary. No root required.
set -eu

REPO="sankh-dev/sankh"
INSTALL_DIR="${SANKH_INSTALL_DIR:-$HOME/.local/bin}"

say() { printf 'sankh-install: %s\n' "$*" >&2; }
die() { say "error: $*"; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "'$1' is required"; }

need curl
need tar
need uname

detect_target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$arch" in
        x86_64 | amd64) arch=x86_64 ;;
        arm64 | aarch64) arch=aarch64 ;;
        *) die "unsupported architecture: $arch" ;;
    esac
    case "$os" in
        Linux)
            if [ "$arch" = x86_64 ] && ldd --version 2>&1 | grep -qi musl; then
                echo "x86_64-unknown-linux-musl"
            else
                echo "$arch-unknown-linux-gnu"
            fi
            ;;
        Darwin) echo "$arch-apple-darwin" ;;
        MINGW* | MSYS* | CYGWIN*) die "on Windows use: powershell -c \"irm https://github.com/$REPO/releases/latest/download/sankh-installer.ps1 | iex\"" ;;
        *) die "unsupported OS: $os" ;;
    esac
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | cut -d' ' -f1
    else
        die "need sha256sum or shasum to verify the download"
    fi
}

target=$(detect_target)
archive="sankh-$target.tar.xz"
if [ -n "${SANKH_VERSION:-}" ]; then
    base="https://github.com/$REPO/releases/download/v${SANKH_VERSION#v}"
else
    base="https://github.com/$REPO/releases/latest/download"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT INT TERM

say "downloading $archive (${SANKH_VERSION:-latest})"
curl -fsSL "$base/$archive" -o "$tmp/$archive" || die "download failed: $base/$archive"
curl -fsSL "$base/$archive.sha256" -o "$tmp/$archive.sha256" || die "checksum download failed"

expected=$(cut -d' ' -f1 <"$tmp/$archive.sha256")
actual=$(sha256 "$tmp/$archive")
[ "$expected" = "$actual" ] || die "checksum mismatch (expected $expected, got $actual)"

tar -xJf "$tmp/$archive" -C "$tmp"
bin=$(find "$tmp" -type f -name sankh | head -n 1)
[ -n "$bin" ] || die "archive did not contain the sankh binary"

mkdir -p "$INSTALL_DIR"
install -m 755 "$bin" "$INSTALL_DIR/sankh" 2>/dev/null || {
    cp "$bin" "$INSTALL_DIR/sankh"
    chmod 755 "$INSTALL_DIR/sankh"
}
say "installed $("$INSTALL_DIR/sankh" --version) to $INSTALL_DIR"

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        say "add $INSTALL_DIR to PATH, e.g.: export PATH=\"$INSTALL_DIR:\$PATH\""
        if [ -n "${GITHUB_PATH:-}" ]; then
            echo "$INSTALL_DIR" >>"$GITHUB_PATH"
            say "added to GITHUB_PATH for later steps"
        fi
        ;;
esac
