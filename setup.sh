#!/usr/bin/env bash
set -euo pipefail

BOLD='\033[1m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }
prompt() { echo -e "${CYAN}[??]${NC} $1"; }

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_DIR"

INSTALL_DIR="${HOME}/.local/bin"
APP_NAME="Ffast-auto-downloader"
BIN_NAME="Auto-FG"
BIN_GET="Auto-FG-get-links"
BIN_DL="Auto-FG-download"
DESKTOP_DIR="${HOME}/.local/share/applications"
DESKTOP_FILE="${DESKTOP_DIR}/Auto-FG.desktop"

# ── detection ────────────────────────────────────
is_installed() { [ -f "${INSTALL_DIR}/${BIN_NAME}" ]; }
have_upstream() { git rev-parse --git-dir &>/dev/null && git remote -v &>/dev/null; }

# ── uninstall ────────────────────────────────────
do_uninstall() {
    echo ""
    info "Removing installed binaries..."
    rm -f "${INSTALL_DIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_GET}" "${INSTALL_DIR}/${BIN_DL}"
    rm -f "${DESKTOP_FILE}"
    info "Uninstalled."
    exit 0
}

# ── update (git pull + rebuild) ──────────────────
do_update() {
    echo ""
    info "Updating from git..."
    git pull --rebase
    info "Rebuilding..."
}
do_reinstall() {
    echo ""
    info "Running full setup..."
}

# ── menu when already installed ──────────────────
menu() {
    echo ""
    echo -e "${BOLD}╔══════════════════════════════════════════╗${NC}"
    echo -e "${BOLD}║         Auto-FG is installed             ║${NC}"
    echo -e "${BOLD}╚══════════════════════════════════════════╝${NC}"
    echo ""
    echo "  1) Update    — git pull + rebuild"
    echo "  2) Reinstall — full setup from scratch"
    echo "  3) Remove    — uninstall binaries"
    echo "  4) Close     — do nothing"
    echo ""
    read -rp "$(prompt "Choose [1-4]: ")" choice
    case "$choice" in
        1) do_update ;;
        2) do_reinstall ;;
        3) do_uninstall ;;
        4) info "Bye."; exit 0 ;;
        *) warn "Invalid choice."; exit 1 ;;
    esac
}

# ── banner ───────────────────────────────────────
echo -e "${BOLD}╔══════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║  FuckingFast FitGirl Download Automator  ║${NC}"
echo -e "${BOLD}║         Automated Setup Script           ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════╝${NC}"

# ── if already installed, show menu ──────────────
if is_installed; then
    menu
fi

# ── 1. Rust ─────────────────────────────────────
install_rust() {
    warn "Rust is not installed."
    echo "  Installing via rustup (https://rustup.rs)..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    if ! command -v cargo &>/dev/null; then
        error "Rust installation failed. Please install manually: https://rustup.rs"
        exit 1
    fi
}

if ! command -v cargo &>/dev/null; then
    install_rust
else
    info "Rust found: $(cargo --version)"
fi

if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

# ── 2. System dependencies ──────────────────────
info "Checking system dependencies..."

if [[ "$(uname)" == "Linux" ]]; then
    if command -v pacman &>/dev/null; then
        info "Arch Linux detected."
        missing=()
        pacman -Qi gtk3 &>/dev/null || missing+=(gtk3)
        pacman -Qi webkit2gtk-4.1 &>/dev/null || missing+=(webkit2gtk-4.1)
        pacman -Qi libunrar &>/dev/null || missing+=(libunrar)
        pacman -Qi base-devel &>/dev/null || missing+=(base-devel)
        if [[ ${#missing[@]} -gt 0 ]]; then
            info "Installing missing packages: ${missing[*]}"
            sudo pacman -S --needed --noconfirm "${missing[@]}"
        fi

    elif command -v apt &>/dev/null; then
        info "Debian/Ubuntu detected."
        sudo apt update
        missing=()
        dpkg -s libgtk-3-dev &>/dev/null 2>&1 || missing+=(libgtk-3-dev)
        dpkg -s libwebkit2gtk-4.1-dev &>/dev/null 2>&1 || missing+=(libwebkit2gtk-4.1-dev)
        dpkg -s libunrar-dev &>/dev/null 2>&1 || missing+=(libunrar-dev)
        dpkg -s build-essential &>/dev/null 2>&1 || missing+=(build-essential)
        dpkg -s pkg-config &>/dev/null 2>&1 || missing+=(pkg-config)
        dpkg -s cmake &>/dev/null 2>&1 || missing+=(cmake)
        if [[ ${#missing[@]} -gt 0 ]]; then
            info "Installing missing packages: ${missing[*]}"
            sudo apt install -y "${missing[@]}"
        fi

    elif command -v dnf &>/dev/null; then
        info "Fedora detected."
        missing=()
        rpm -q gtk3-devel &>/dev/null || missing+=(gtk3-devel)
        rpm -q webkit2gtk4.1-devel &>/dev/null || missing+=(webkit2gtk4.1-devel)
        rpm -q libunrar-devel &>/dev/null || missing+=(libunrar-devel)
        rpm -q gcc &>/dev/null || missing+=(gcc)
        rpm -q pkg-config &>/dev/null || missing+=(pkg-config)
        rpm -q cmake &>/dev/null || missing+=(cmake)
        if [[ ${#missing[@]} -gt 0 ]]; then
            info "Installing missing packages: ${missing[*]}"
            sudo dnf install -y "${missing[@]}"
        fi

    else
        warn "Unknown package manager. Please install these manually:"
        echo "  - gtk3 + development headers"
        echo "  - webkit2gtk + development headers"
        echo "  - libunrar + development headers"
        echo "  - build-essential (gcc, pkg-config, cmake)"
    fi

    if ! command -v unrar &>/dev/null && ! command -v unrar-free &>/dev/null; then
        warn "Neither 'unrar' nor 'unrar-free' found. RAR extraction may fail."
        echo "  Install 'unrar' via your package manager."
    else
        info "RAR extraction tool found."
    fi

    info "System dependencies OK."

elif [[ "$(uname)" == "Darwin" ]]; then
    info "macOS detected."
    if ! command -v brew &>/dev/null; then
        warn "Homebrew not found. Some libraries may be missing."
        echo "  Install from: https://brew.sh"
    fi
fi

# ── 3. Git ──────────────────────────────────────
if git rev-parse --git-dir &>/dev/null; then
    info "Git repository detected."
    git config --global --add safe.directory "$PROJECT_DIR" 2>/dev/null || true
    git submodule update --init --recursive 2>/dev/null || true
    info "Git repository configured."
else
    warn "Not a git repository. Skipping git setup."
fi

# ── 4. Build ────────────────────────────────────
echo ""
info "Building project (release mode)..."
cargo build --release

# ── 5. Install ──────────────────────────────────
mkdir -p "$INSTALL_DIR"
echo ""
info "Installing binaries to ${INSTALL_DIR}..."

install_binary() {
    local src="$1"
    local name="$2"
    local dest="${INSTALL_DIR}/${name}"
    if [ -f "$src" ]; then
        if [ -d "$dest" ]; then
            rm -rf "$dest"
        fi
        cp "$src" "$dest"
        chmod +x "$dest"
        info "  Installed: ${name}"
    fi
}

install_binary "target/release/${APP_NAME}"   "${BIN_NAME}"
install_binary "target/release/get-links"      "${BIN_GET}"
install_binary "target/release/download"       "${BIN_DL}"

if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    warn "~/.local/bin is not in your PATH."
    echo "  Add this to your shell config (~/.bashrc, ~/.zshrc, etc.):"
    echo '    export PATH="$HOME/.local/bin:$PATH"'
fi

# ── 6. Desktop entry ───────────────────────────
if [[ "$(uname)" == "Linux" ]]; then
    mkdir -p "$DESKTOP_DIR"
    cat > "$DESKTOP_FILE" << EOF
[Desktop Entry]
Version=1.0
Name=Auto-FG
Comment=Automate downloading and extracting FitGirl repacks
Exec=${INSTALL_DIR}/${BIN_NAME}
Icon=applications-games
Terminal=false
Type=Application
Categories=Game;Utility;
StartupWMClass=Ffast-auto-downloader
EOF
    info "Desktop entry created: ${DESKTOP_FILE}"
fi

# ── 7. Done ─────────────────────────────────────
echo ""
echo -e "${BOLD}╔══════════════════════════════════════════╗${NC}"
echo -e "${BOLD}║           Setup Complete!                 ║${NC}"
echo -e "${BOLD}╚══════════════════════════════════════════╝${NC}"
echo ""
echo "  Commands:"
echo "    ${BIN_NAME}             — Launch GUI"
echo "    ${BIN_GET}              — CLI link scraper"
echo "    ${BIN_DL}               — CLI batch downloader"
echo ""
echo "  Run:  ${BIN_NAME}"
echo "  Rerun this script to update / reinstall / remove."
echo ""
