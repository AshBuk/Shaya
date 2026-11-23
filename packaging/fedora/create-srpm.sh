#!/bin/bash
# Copyright 2025 Asher Buk
#
# SPDX-License-Identifier: Apache-2.0
#
# https://github.com/AshBuk/Shaya
#
# Build an SRPM from the working tree with vendored crates.
# Usage: create-srpm.sh [topdir], default topdir is ~/rpmbuild

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SPEC="$ROOT/packaging/fedora/shaya.spec"
TOPDIR="$(mkdir -p "${1:-$HOME/rpmbuild}" && cd "${1:-$HOME/rpmbuild}" && pwd)"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
if ! grep -qE "^Version:[[:space:]]+$VERSION$" "$SPEC"; then
    echo "error: shaya.spec Version does not match Cargo.toml ($VERSION)" >&2
    exit 1
fi

SOURCES="$TOPDIR/SOURCES"
mkdir -p "$SOURCES"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

cd "$ROOT"
# Same top directory as the GitHub tag tarball named in Source0
git ls-files -z |
    tar --null -T - --transform "s,^,Shaya-$VERSION/," -czf "$SOURCES/shaya-$VERSION.tar.gz"

cargo vendor --locked "$TMP/vendor" >/dev/null
tar -C "$TMP" -cJf "$SOURCES/shaya-$VERSION-vendor.tar.xz" vendor

rpmbuild -bs --define "_topdir $TOPDIR" "$SPEC"
