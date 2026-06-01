#!/usr/bin/env bash
set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

info()  { echo -e "${GREEN}[INFO]${NC} $1"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; }

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$PROJECT_DIR"

INSTALL_DIR="${HOME}/.local/bin"
APP_NAME="Ffast-auto-downloader"
BIN_NAME="Auto-FG"
BIN_GET="Auto-FG-get-links"
BIN_DL="Auto-FG-download"
DESKTOP_DIR="${HOME}/.local/share/applications"
DESKTOP_FILE="${DESKTOP_DIR}/Auto-FG.desktop"

echo "╔══════════════════════════════════════════╗"
echo "║     Auto-FG Setup                        ║"
echo "╚══════════════════════════════════════════╝"
echo ""

# ── 1. Install Rust if missing ────────────────
if ! command -v cargo &>/dev/null; then
    warn "Rust not found. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
source "$HOME/.cargo/env" 2>/dev/null || true
info "Rust: $(cargo --version)"

# ── 2. System dependencies ────────────────────
if [[ "$(uname)" == "Linux" ]]; then
    if command -v pacman &>/dev/null; then
        sudo pacman -S --needed --noconfirm gtk3 webkit2gtk-4.1 libunrar base-devel 2>/dev/null
    elif command -v apt &>/dev/null; then
        sudo apt update && sudo apt install -y libgtk-3-dev libwebkit2gtk-4.1-dev libunrar-dev build-essential pkg-config cmake
    elif command -v dnf &>/dev/null; then
        sudo dnf install -y gtk3-devel webkit2gtk4.1-devel libunrar-devel gcc pkg-config cmake
    fi
fi

# ── 3. Build ──────────────────────────────────
info "Building (this may take a while)..."
cargo build --release

# ── 4. Install ────────────────────────────────
mkdir -p "$INSTALL_DIR"
cp "target/release/${APP_NAME}" "${INSTALL_DIR}/${BIN_NAME}"
cp "target/release/get-links"   "${INSTALL_DIR}/${BIN_GET}"
cp "target/release/download"    "${INSTALL_DIR}/${BIN_DL}"
chmod +x "${INSTALL_DIR}/${BIN_NAME}" "${INSTALL_DIR}/${BIN_GET}" "${INSTALL_DIR}/${BIN_DL}"

# ── 5. Desktop shortcut ──────────────────────
mkdir -p "$DESKTOP_DIR"
cat > "$DESKTOP_FILE" << EOF
[Desktop Entry]
Version=1.0
Name=Auto-FG
Comment=FitGirl repack downloader
Exec=${INSTALL_DIR}/${BIN_NAME}
Terminal=false
Type=Application
Categories=Game;Utility;
EOF

# also put a shortcut on the desktop
if [ -d "${HOME}/Desktop" ]; then
    cp "$DESKTOP_FILE" "${HOME}/Desktop/"
    chmod +x "${HOME}/Desktop/Auto-FG.desktop"
    info "Desktop shortcut created"
fi

# ensure ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> "${HOME}/.bashrc"
    echo 'export PATH="$HOME/.local/bin:$PATH"' >> "${HOME}/.zshrc" 2>/dev/null || true
    info "Added ~/.local/bin to PATH (restart shell or run: source ~/.bashrc)"
fi

# ── 6. Done ─────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════╗"
echo "║           All Set!                       ║"
echo "╚══════════════════════════════════════════╝"
echo ""
echo "  Run it:  ${BIN_NAME}"
echo "  Or click the desktop shortcut"
echo "  Re-run this script to update"
echo ""
