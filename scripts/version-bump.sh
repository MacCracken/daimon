#!/bin/sh
# version-bump.sh <version> — the ONE place a daimon version number changes.
#
# Two files carry it and they must never disagree:
#   VERSION         — the source of truth; cyrius.cyml reads it via ${file:VERSION}
#   src/config.cyr  — `var DAIMON_VERSION`, COMPILED IN so the binary reports its
#                     own version from any cwd (2.1.7; before that daimon read the
#                     VERSION file at runtime and reported "unknown" anywhere but
#                     the repo root — found by booting the agnos build).
#
# tests/version_sync.tcyr fails the suite if they drift, so a hand-edit of one
# without the other is caught by CI rather than by a user reading a wrong banner.
set -e
VERSION="$1"
if [ -z "$VERSION" ]; then
    echo "Usage: ./scripts/version-bump.sh <version>"
    exit 1
fi
echo "$VERSION" > VERSION
sed -i "s/^var DAIMON_VERSION = \".*\";$/var DAIMON_VERSION = \"$VERSION\";/" src/config.cyr
echo "VERSION        -> $VERSION"
echo "DAIMON_VERSION -> $(grep -oE '^var DAIMON_VERSION = "[^"]*"' src/config.cyr | sed 's/.*"\(.*\)"/\1/')"
echo "(cyrius.cyml resolves [package].version from VERSION via \${file:VERSION})"
