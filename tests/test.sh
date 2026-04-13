#!/bin/sh
# daimon test runner
# Usage: sh tests/test.sh
set -e

echo "=== daimon tests ==="
mkdir -p build
cyrius build tests/daimon.tcyr build/daimon_test
build/daimon_test
TEST_EXIT=$?
rm -f build/daimon_test

echo ""
echo "=== fuzz harnesses ==="
for f in fuzz/*.fcyr; do
    name=$(basename "$f" .fcyr)
    printf "  %s: " "$name"
    cyrius build "$f" "build/fz_$name" 2>/dev/null && timeout 5 "build/fz_$name" && echo "PASS" || echo "FAIL"
    rm -f "build/fz_$name"
done

exit $TEST_EXIT
