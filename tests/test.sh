#!/bin/sh
CC="${1:-./build/cc3}"
echo "=== daimon tests ==="
cat src/main.cyr | "$CC" > /tmp/daimon_test && chmod +x /tmp/daimon_test && /tmp/daimon_test
echo "exit: $?"
rm -f /tmp/daimon_test
