#!/usr/bin/env bash

echo "This test takes about 30s"
( cd ../.. ; cargo build )
rm -rf data
../../target/debug/sonar daemon daemon-directory.ini
n=$(wc -l $(find data -name '0+sysinfo*json') | head -n 1 | awk '{ print $1 }')
if (( $n == 0 )); then
    echo "Sysinfo file should not be empty"
    exit 1
fi
n=$(wc -l $(find data -name '0+sample*json') | head -n 1 | awk '{ print $1 }')
if (( $n == 0 )); then
    echo "Sample file should not be empty"
    exit 1
fi
echo "OK"
exit 0

