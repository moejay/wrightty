#!/bin/sh
# Install wrightty — terminal automation CLI
# Usage: curl -fsSL https://raw.githubusercontent.com/moejay/wrightty/main/install.sh | sh
set -e

REPO="moejay/wrightty"
INSTALL_DIR="${WRIGHTTY_INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Linux)  OS_NAME="linux" ;;
  Darwin) OS_NAME="macos" ;;
  *)
    echo "error: Unsupported OS: $OS"
    echo "Build from source: cargo install --git https://github.com/$REPO wrightty"
    exit 1
    ;;
esac

case "$ARCH" in
  x86_64|amd64)  ARCH_NAME="x86_64" ;;
  aarch64|arm64) ARCH_NAME="aarch64" ;;
  *)
    echo "error: Unsupported architecture: $ARCH"
    echo "Build from source: cargo install --git https://github.com/$REPO wrightty"
    exit 1
    ;;
esac

ASSET_NAME="wrightty-${OS_NAME}-${ARCH_NAME}"

# Get latest release URL
if [ -n "$WRIGHTTY_VERSION" ]; then
  TAG="v${WRIGHTTY_VERSION}"
  URL="https://github.com/$REPO/releases/download/$TAG/${ASSET_NAME}.tar.gz"
else
  URL="https://github.com/$REPO/releases/latest/download/${ASSET_NAME}.tar.gz"
fi

echo "Downloading wrightty for ${OS_NAME}/${ARCH_NAME}..."
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$URL" -o "$TMPDIR/wrightty.tar.gz"
elif command -v wget >/dev/null 2>&1; then
  wget -q "$URL" -O "$TMPDIR/wrightty.tar.gz"
else
  echo "error: curl or wget required"
  exit 1
fi

tar xzf "$TMPDIR/wrightty.tar.gz" -C "$TMPDIR"

# Install
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMPDIR/wrightty" "$INSTALL_DIR/wrightty"
else
  echo "Installing to $INSTALL_DIR (requires sudo)..."
  sudo mv "$TMPDIR/wrightty" "$INSTALL_DIR/wrightty"
fi

chmod +x "$INSTALL_DIR/wrightty"

echo ""
echo "wrightty installed to $INSTALL_DIR/wrightty"
echo ""
echo "Get started:"
echo "  wrightty term --headless        # start a headless terminal server"
echo "  wrightty run \"echo hello\"       # run a command"
echo "  wrightty discover               # find running servers"
echo ""
echo "Or bridge to your terminal:"
echo "  wrightty term --bridge-tmux     # if you use tmux"
echo "  wrightty term --bridge-wezterm  # if you use WezTerm"
echo "  wrightty term --bridge-kitty    # if you use Kitty"
echo "  wrightty term --bridge-ghostty  # if you use Ghostty"
echo "  wrightty term --bridge-zellij   # if you use Zellij"
