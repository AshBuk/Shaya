#!/bin/bash
# Copyright 2025 Asher Buk
#
# SPDX-License-Identifier: Apache-2.0
#
# https://github.com/AshBuk/Shaya
#
# Build the AppImage in an Arch Linux container, as root.
# quick-sharun bundles the binary with every library it loads, glibc and
# Mesa included, so the AppImage runs on any distribution.
# Usage: build.sh, the AppImage and its .zsync land in dist/

set -euo pipefail

# The tools live on a branch that changes daily: pin the commit and the files.
# quick-sharun checks its own downloads (sharun, appimagetool) by sha256.
ANYLINUX_COMMIT=63759aceda3a0cb9446a4e3c2aebe3af5e5c697d
QUICK_SHARUN_SHA256=77392d48d845708c7787006b7d86ab73b4d022a2d4d5a4cd6887705a3c4dbbe1
DEBLOATED_PKGS_SHA256=d215f7676785f1a1bb8784e69085d66c1df81488ef31f1ba7198325a5be05130

APP_ID=io.github.ashbuk.shaya

cd "$(dirname "${BASH_SOURCE[0]}")/../.."
ROOT="$PWD"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
ARCH="$(uname -m)"
TOOLS="$(mktemp -d)"
trap 'rm -rf "$TOOLS"' EXIT

# Packages the anylinux setup action installs, plus the build dependencies
pacman -Syu --noconfirm --needed \
    7zip base-devel curl freetype2 git jq patchelf unzip wget xorg-server-xvfb zsync \
    rust gtk4 libadwaita gtk4-layer-shell

fetch() {
    curl -fsSL --retry 3 -o "$TOOLS/$1" \
        "https://raw.githubusercontent.com/pkgforge-dev/Anylinux-AppImages/$ANYLINUX_COMMIT/useful-tools/$1.sh"
    echo "$2  $TOOLS/$1" | sha256sum -c -
    chmod +x "$TOOLS/$1"
}
fetch quick-sharun "$QUICK_SHARUN_SHA256"
fetch get-debloated-pkgs "$DEBLOATED_PKGS_SHA256"

# Smaller GTK and Mesa builds. Has to run after every pacman -S.
"$TOOLS/get-debloated-pkgs" --add-common --prefer-nano

cargo build --release --locked

# The AppImage spec looks the icon up by the Icon= key of the desktop entry
cp icons/shaya.png "$TOOLS/$APP_ID.png"

export ARCH VERSION
export APPDIR="$ROOT/target/AppDir"
export OUTPATH="$ROOT/dist"
export OUTNAME="Shaya-$VERSION-$ARCH.AppImage"
export DESKTOP="$ROOT/assets/$APP_ID.desktop"
export ICON="$TOOLS/$APP_ID.png"
export UPINFO="gh-releases-zsync|AshBuk|Shaya|latest|Shaya-*-$ARCH.AppImage.zsync"

rm -rf "$APPDIR"
"$TOOLS/quick-sharun" target/release/shaya
"$TOOLS/quick-sharun" --make-appimage
"$TOOLS/quick-sharun" --test "$OUTPATH/$OUTNAME"
