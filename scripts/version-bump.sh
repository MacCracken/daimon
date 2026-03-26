#!/bin/bash
set -euo pipefail
VERSION="$1"
sed -i "s/^version = .*/version = \"$VERSION\"/" Cargo.toml
echo "$VERSION" > VERSION
