#!/bin/sh
set -e
mkdir -p build
cyrius build tests/daimon.bcyr build/daimon_bench
build/daimon_bench | tee -a bench-history.csv
rm -f build/daimon_bench
