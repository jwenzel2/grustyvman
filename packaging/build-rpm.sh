#!/bin/bash
# Build a binary RPM for grustyvman.
# Run from anywhere; the script locates the repo root automatically.

set -euo pipefail

VERSION="1.0"
NAME="grustyvman"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SPEC="${REPO_ROOT}/packaging/${NAME}.spec"

echo "==> Setting up rpmbuild tree"
mkdir -p ~/rpmbuild/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}

echo "==> Creating source tarball (excluding .git and target/)"
tar -czf ~/rpmbuild/SOURCES/${NAME}-${VERSION}.tar.gz \
    --exclude='.git' \
    --exclude='target' \
    --exclude='viewer/target' \
    --transform "s,^\.,${NAME}-${VERSION}," \
    -C "${REPO_ROOT}" .

echo "==> Copying spec"
cp "${SPEC}" ~/rpmbuild/SPECS/${NAME}.spec

echo "==> Running rpmbuild"
rpmbuild -bb ~/rpmbuild/SPECS/${NAME}.spec

echo ""
echo "==> Done! RPM:"
find ~/rpmbuild/RPMS -name "${NAME}-${VERSION}*.rpm" | sort | tail -1
