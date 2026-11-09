#!/bin/bash
# Copyright 2025 Asher Buk
#
# SPDX-License-Identifier: Apache-2.0
#
# https://github.com/AshBuk/Shaya
#
# Fail unless every file that carries the version names the release.
# Usage: check-version.sh X.Y.Z

set -euo pipefail

version="$1"
cd "$(dirname "${BASH_SOURCE[0]}")/.."

rc=0
check() {
    if ! grep -qE "$2" "$1"; then
        echo "error: $1 does not name version $version" >&2
        rc=1
    fi
}

check Cargo.toml "^version = \"$version\"$"
check packaging/fedora/shaya.spec "^Version:[[:space:]]+$version$"
check packaging/arch/PKGBUILD "^pkgver=$version$"
check assets/io.github.ashbuk.shaya.metainfo.xml "<release version=\"$version\""

exit "$rc"
