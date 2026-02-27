#!/bin/bash
# Build a binary Arch Linux package for grustyvman.
# Run from anywhere; the script locates the repo root automatically.

set -euo pipefail

VERSION="1.8.0"
NAME="grustyvman"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="${REPO_ROOT}/packaging/build"

mkdir -p "${BUILD_DIR}"

echo "==> Creating source tarball (excluding .git and target/)"
tar -czf "${BUILD_DIR}/${NAME}-${VERSION}.tar.gz" \
    --exclude='.git' \
    --exclude='./target' \
    --exclude='./viewer/target' \
    --exclude='./packaging/build' \
    --transform "s,^\.,${NAME}-${VERSION}," \
    -C "${REPO_ROOT}" .

echo "==> Copying PKGBUILD"
cp "${REPO_ROOT}/packaging/PKGBUILD" "${BUILD_DIR}/PKGBUILD"

echo "==> Running makepkg"
cd "${BUILD_DIR}"
makepkg -sf

echo ""
echo "==> Done! Package:"
find "${BUILD_DIR}" -maxdepth 1 -name "${NAME}-*.pkg.tar.*" | sort | tail -1

echo ""
echo "==> To install, run:"
find "${BUILD_DIR}" -maxdepth 1 -name "${NAME}-*.pkg.tar.*" | sort | tail -1 | xargs echo "sudo pacman -U"
