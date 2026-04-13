#!/bin/sh
set -e
CC="${1:-./build/cc3}"
cat src/main.cyr | "$CC" > /tmp/daimon_bench && chmod +x /tmp/daimon_bench
/tmp/daimon_bench bench 2>/dev/null | tee -a bench-history.csv
rm -f /tmp/daimon_bench
