#!/bin/bash
set -e

# Nagisa PDF One-Line Installer
REPO="AI-SLOP-BOX/nagisa-pdf"
OS="$(uname -s)"
ARCH="$(uname -m)"

echo "Nagisa PDF Installer"
echo "======================"
echo "Detected Platform: $OS ($ARCH)"

case "$OS" in
  Darwin*)
    echo "Downloading Nagisa PDF for macOS..."
    LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    if [ -z "$LATEST_TAG" ]; then
      LATEST_TAG="v1.0.0"
    fi
    DMG_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/Nagisa_${ARCH}.dmg"
    echo "URL: $DMG_URL"
    rm -f /tmp/Nagisa.dmg /tmp/Nagisa.dmg.tmp
    if curl -fSL -o /tmp/Nagisa.dmg.tmp "$DMG_URL"; then
      mv /tmp/Nagisa.dmg.tmp /tmp/Nagisa.dmg
      hdiutil attach /tmp/Nagisa.dmg -nobrowse -mountpoint /Volumes/Nagisa
      cp -R "/Volumes/Nagisa/Nagisa.app" /Applications/
      hdiutil detach /Volumes/Nagisa
      rm -f /tmp/Nagisa.dmg
      echo "Nagisa PDF installed successfully to /Applications/Nagisa.app"
    else
      rm -f /tmp/Nagisa.dmg.tmp
      echo "Pre-built binary not found. Running local build fallback..."
      git clone "https://github.com/$REPO.git"
      cd nagisa-pdf && ./build.sh
    fi
    ;;
  Linux*)
    echo "Downloading Nagisa PDF for Linux (AppImage)..."
    LATEST_TAG=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    if [ -z "$LATEST_TAG" ]; then
      LATEST_TAG="v1.0.0"
    fi
    APPIMAGE_URL="https://github.com/$REPO/releases/download/$LATEST_TAG/Nagisa_x86_64.AppImage"
    rm -f /tmp/Nagisa.AppImage /tmp/Nagisa.AppImage.tmp
    if curl -fSL -o /tmp/Nagisa.AppImage.tmp "$APPIMAGE_URL"; then
      mv /tmp/Nagisa.AppImage.tmp /tmp/Nagisa.AppImage
      chmod +x /tmp/Nagisa.AppImage
      mkdir -p "$HOME/.local/bin"
      mv /tmp/Nagisa.AppImage "$HOME/.local/bin/nagisa"
      echo "Nagisa PDF installed to $HOME/.local/bin/nagisa"
    else
      rm -f /tmp/Nagisa.AppImage.tmp
      echo "Pre-built binary not found. Running local build fallback..."
      git clone "https://github.com/$REPO.git"
      cd nagisa-pdf && ./build.sh
    fi
    ;;
  *)
    echo "Unsupported OS: $OS. Please install from https://github.com/$REPO/releases"
    exit 1
    ;;
esac
