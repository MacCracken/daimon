#!/bin/sh
# daimon test runner
# Usage: sh tests/test.sh [cc3-path]
set -e
CC="${1:-cc3}"

echo "=== daimon tests ==="
"$CC" < tests/daimon.tcyr > /tmp/daimon_test && chmod +x /tmp/daimon_test && /tmp/daimon_test
TEST_EXIT=$?
rm -f /tmp/daimon_test

echo ""
echo "=== fuzz harnesses ==="
for f in fuzz/*.fcyr; do
    name=$(basename "$f" .fcyr)
    printf "  %s: " "$name"
    "$CC" < "$f" > "/tmp/fz_$name" 2>/dev/null && chmod +x "/tmp/fz_$name" && "/tmp/fz_$name" && echo "PASS" || echo "FAIL"
    rm -f "/tmp/fz_$name"
done

exit $TEST_EXIT
