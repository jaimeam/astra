#!/bin/sh
# Astra binary installer — downloads a pre-built release from GitHub.
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/jaimeam/astra/main/install-binary.sh | sh
#   ASTRA_VERSION=v1.0.0 curl -fsSL ... | sh
set -eu

REPO="jaimeam/astra"
INSTALL_DIR="${ASTRA_HOME:-$HOME/.astra}/bin"

main() {
    detect_platform
    get_latest_version
    download_and_install
    setup_path
    verify_install
}

detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux)  OS_TARGET="unknown-linux-gnu" ;;
        Darwin) OS_TARGET="apple-darwin" ;;
        *)      error "Unsupported OS: $OS. See https://github.com/$REPO/releases for manual download." ;;
    esac

    case "$ARCH" in
        x86_64|amd64)  ARCH_TARGET="x86_64" ;;
        aarch64|arm64) ARCH_TARGET="aarch64" ;;
        *)             error "Unsupported architecture: $ARCH" ;;
    esac

    TARGET="${ARCH_TARGET}-${OS_TARGET}"
    info "Detected platform: $TARGET"
}

get_latest_version() {
    if [ -n "${ASTRA_VERSION:-}" ]; then
        VERSION="$ASTRA_VERSION"
    else
        VERSION="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
            | grep '"tag_name"' | head -1 | cut -d'"' -f4)" || true
    fi

    if [ -z "$VERSION" ]; then
        error "Could not determine latest version. Set ASTRA_VERSION to install a specific release."
    fi

    info "Installing Astra $VERSION"
}

download_and_install() {
    ARCHIVE="astra-${VERSION}-${TARGET}.tar.gz"
    URL="https://github.com/$REPO/releases/download/${VERSION}/${ARCHIVE}"
    CHECKSUM_URL="${URL}.sha256"

    TMPDIR="$(mktemp -d)"
    trap 'rm -rf "$TMPDIR"' EXIT

    info "Downloading $URL"
    curl -fsSL "$URL" -o "$TMPDIR/$ARCHIVE" \
        || error "Download failed. Check that $VERSION has a release for $TARGET at https://github.com/$REPO/releases"

    # Verify checksum if shasum is available
    if command -v shasum >/dev/null 2>&1; then
        info "Verifying checksum..."
        curl -fsSL "$CHECKSUM_URL" -o "$TMPDIR/checksum.sha256" 2>/dev/null || true
        if [ -f "$TMPDIR/checksum.sha256" ]; then
            EXPECTED="$(awk '{print $1}' "$TMPDIR/checksum.sha256")"
            ACTUAL="$(shasum -a 256 "$TMPDIR/$ARCHIVE" | awk '{print $1}')"
            if [ "$EXPECTED" != "$ACTUAL" ]; then
                error "Checksum mismatch! Expected: $EXPECTED, Got: $ACTUAL"
            fi
            info "Checksum verified."
        fi
    fi

    # Extract and install
    tar xzf "$TMPDIR/$ARCHIVE" -C "$TMPDIR"
    mkdir -p "$INSTALL_DIR"
    cp "$TMPDIR/astra-${VERSION}-${TARGET}/astra" "$INSTALL_DIR/astra"
    chmod +x "$INSTALL_DIR/astra"
    info "Installed to $INSTALL_DIR/astra"
}

setup_path() {
    case ":${PATH}:" in
        *":$INSTALL_DIR:"*) return ;;
    esac

    SHELL_NAME="$(basename "${SHELL:-/bin/sh}")"
    case "$SHELL_NAME" in
        zsh)  PROFILE="$HOME/.zshrc" ;;
        bash) PROFILE="$HOME/.bashrc" ;;
        fish) PROFILE="$HOME/.config/fish/config.fish" ;;
        *)    PROFILE="$HOME/.profile" ;;
    esac

    EXPORT_LINE="export PATH=\"$INSTALL_DIR:\$PATH\""
    if [ "$SHELL_NAME" = "fish" ]; then
        EXPORT_LINE="set -gx PATH $INSTALL_DIR \$PATH"
    fi

    if [ -f "$PROFILE" ] && grep -qF "$INSTALL_DIR" "$PROFILE" 2>/dev/null; then
        return
    fi

    printf '\n# Astra programming language\n%s\n' "$EXPORT_LINE" >> "$PROFILE"
    info "Added $INSTALL_DIR to PATH in $PROFILE"
    info "Restart your shell or run: $EXPORT_LINE"
}

verify_install() {
    if "$INSTALL_DIR/astra" --version >/dev/null 2>&1; then
        VERSION_OUT="$("$INSTALL_DIR/astra" --version)"
        info ""
        info "Astra installed successfully! ($VERSION_OUT)"
        info "Run 'astra init my-project' to get started."
    else
        warn "Binary installed but could not verify. Restart your shell and try: astra --version"
    fi
}

info()  { printf '  \033[1;32m>\033[0m %s\n' "$1"; }
warn()  { printf '  \033[1;33m!\033[0m %s\n' "$1"; }
error() { printf '  \033[1;31mx\033[0m %s\n' "$1" >&2; exit 1; }

main
