#!/bin/sh
set -e
VERSION="$1"
if [ -z "$VERSION" ]; then
    echo "Usage: ./scripts/version-bump.sh <version>"
    exit 1
fi
sed -i "s/^version = .*/version = \"$VERSION\"/" cyrius.toml
echo "$VERSION" > VERSION
