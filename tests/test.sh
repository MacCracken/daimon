#!/bin/sh
# daimon test runner
# Usage: sh tests/test.sh
set -e

echo "=== daimon tests ==="
# Every suite, not just daimon.tcyr. Through 2.2.2 this built daimon.tcyr alone,
# so as the test-integrity arc moved assertions into per-module suites (agent,
# supervisor, ipc, http, … — each including the real src/ file) this runner
# silently stopped running them. CI already loops over tests/*.tcyr; now the
# local runner matches it.
mkdir -p build
TEST_EXIT=0
for t in tests/*.tcyr; do
    [ -f "$t" ] || continue
    if ! cyrius test "$t"; then
        echo "FAIL: $t"
        TEST_EXIT=1
    fi
done

echo ""
# The HTTP smoke checks against the LINKED binary — their own script since
# 2.2.3 so CI runs them too (they had rotted twice while only this runner did).
sh tests/smoke.sh || TEST_EXIT=1

echo ""
echo "=== fuzz harnesses ==="
# A failing harness now fails the run (it used to print FAIL and exit 0).
cyrius fuzz || TEST_EXIT=1

exit $TEST_EXIT
