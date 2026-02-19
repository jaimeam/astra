#!/usr/bin/env bash
set -euo pipefail

# ─── Astra install script ───────────────────────────────────────────────
# Sets up everything needed to build and develop Astra on a fresh machine.
# Supports Linux (Debian/Ubuntu, Fedora/RHEL, Arch, Alpine), macOS, and WSL.
# ─────────────────────────────────────────────────────────────────────────

RUST_TOOLCHAIN="stable"
MIN_RUST_VERSION="1.70.0"

# ─── Helpers ─────────────────────────────────────────────────────────────

info()  { printf '\033[1;34m[info]\033[0m  %s\n' "$*"; }
ok()    { printf '\033[1;32m[ok]\033[0m    %s\n' "$*"; }
warn()  { printf '\033[1;33m[warn]\033[0m  %s\n' "$*"; }
err()   { printf '\033[1;31m[error]\033[0m %s\n' "$*"; exit 1; }

need_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        return 1
    fi
    return 0
}

# Compare semver: returns 0 if $1 >= $2
version_ge() {
    printf '%s\n%s' "$1" "$2" | sort -V | head -n1 | grep -qx "$2"
}

detect_os() {
    case "$(uname -s)" in
        Linux*)  OS="linux" ;;
        Darwin*) OS="macos" ;;
        MINGW*|MSYS*|CYGWIN*) OS="windows" ;;
        *)       err "Unsupported OS: $(uname -s)" ;;
    esac
}

detect_distro() {
    if [ "$OS" != "linux" ]; then
        DISTRO=""
        return
    fi
    if [ -f /etc/os-release ]; then
        # shellcheck disable=SC1091
        . /etc/os-release
        case "$ID" in
            ubuntu|debian|pop|linuxmint|elementary) DISTRO="debian" ;;
            fedora|rhel|centos|rocky|alma)          DISTRO="fedora" ;;
            arch|manjaro|endeavouros)               DISTRO="arch" ;;
            alpine)                                 DISTRO="alpine" ;;
            *)                                      DISTRO="unknown" ;;
        esac
    else
        DISTRO="unknown"
    fi
}

# ─── System dependencies ────────────────────────────────────────────────

install_system_deps() {
    info "Installing system dependencies..."

    case "$OS" in
        macos)
            if ! need_cmd brew; then
                warn "Homebrew not found. Installing..."
                /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
            fi
            # Xcode CLT provides cc, make, git
            if ! xcode-select -p >/dev/null 2>&1; then
                info "Installing Xcode Command Line Tools..."
                xcode-select --install
                echo "Press enter after Xcode CLT installation finishes."
                read -r
            fi
            # Ensure git and curl are available (they come with Xcode CLT)
            for cmd in git curl; do
                need_cmd "$cmd" || brew install "$cmd"
            done
            ;;
        linux)
            case "$DISTRO" in
                debian)
                    sudo apt-get update -y
                    sudo apt-get install -y build-essential curl git pkg-config libssl-dev
                    ;;
                fedora)
                    sudo dnf groupinstall -y "Development Tools"
                    sudo dnf install -y curl git gcc pkg-config openssl-devel
                    ;;
                arch)
                    sudo pacman -Sy --noconfirm --needed base-devel curl git openssl pkg-config
                    ;;
                alpine)
                    sudo apk add --no-cache build-base curl git openssl-dev pkgconfig
                    ;;
                *)
                    warn "Unknown Linux distro. Please ensure these are installed:"
                    warn "  - C compiler (gcc/clang), make, curl, git, pkg-config, OpenSSL dev headers"
                    ;;
            esac
            ;;
        windows)
            warn "On Windows, ensure you have Visual Studio Build Tools installed."
            warn "See: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
            need_cmd git  || err "git is required. Install Git for Windows: https://git-scm.com"
            need_cmd curl || err "curl is required."
            ;;
    esac

    ok "System dependencies ready."
}

# ─── Rust toolchain ─────────────────────────────────────────────────────

install_rust() {
    if need_cmd rustc && need_cmd cargo; then
        local current_version
        current_version="$(rustc --version | awk '{print $2}')"
        if version_ge "$current_version" "$MIN_RUST_VERSION"; then
            ok "Rust $current_version already installed (>= $MIN_RUST_VERSION)."
        else
            warn "Rust $current_version is too old (need >= $MIN_RUST_VERSION). Updating..."
            rustup update "$RUST_TOOLCHAIN"
        fi
    else
        info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain "$RUST_TOOLCHAIN"
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi

    # Ensure required components
    info "Ensuring rustfmt and clippy are installed..."
    rustup component add rustfmt clippy 2>/dev/null || true

    ok "Rust toolchain ready: $(rustc --version)"
}

# ─── Project setup ───────────────────────────────────────────────────────

setup_project() {
    local project_dir
    project_dir="$(cd "$(dirname "$0")" && pwd)"

    info "Setting up Astra project in $project_dir..."

    # Git hooks
    info "Configuring git hooks..."
    git -C "$project_dir" config core.hooksPath .githooks
    ok "Git hooks configured."

    # Build
    info "Building Astra (debug)..."
    cargo build --manifest-path "$project_dir/Cargo.toml"
    ok "Build succeeded."

    # Tests
    info "Running tests..."
    cargo test --manifest-path "$project_dir/Cargo.toml"
    ok "All tests passed."
}

# ─── Main ────────────────────────────────────────────────────────────────

main() {
    echo ""
    echo "  ╔═══════════════════════════════════════╗"
    echo "  ║       Astra — Install & Setup         ║"
    echo "  ╚═══════════════════════════════════════╝"
    echo ""

    detect_os
    detect_distro
    info "Detected: OS=$OS${DISTRO:+, distro=$DISTRO}"

    install_system_deps
    install_rust
    setup_project

    echo ""
    ok "Astra is ready! Try:"
    echo ""
    echo "    cargo run -- run examples/hello.astra"
    echo "    cargo run -- check examples/hello.astra"
    echo "    cargo run -- test"
    echo ""
}

main "$@"
