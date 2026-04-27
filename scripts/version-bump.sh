#!/bin/sh
set -e
VERSION="$1"
if [ -z "$VERSION" ]; then
    echo "Usage: ./scripts/version-bump.sh <version>"
    exit 1
fi
echo "$VERSION" > VERSION
# cyrius.cyml resolves [package].version from VERSION via ${file:VERSION}.
