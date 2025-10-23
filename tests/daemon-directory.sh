#!/usr/bin/env bash
#
# Collect data into a directory for 20s and then check that data were collected properly.
#
# TODO: We should extend this to check that timestamps and file names are correct.

source sh-helper
assert cargo

echo "This test takes about 20s"

data_dir=tmp/daemon-directory-data
logfile=tmp/daemon-directory-log.txt
rm -rf $data_dir $logfile

cargo run -- daemon daemon-directory.ini 2>$logfile
if [[ ! -d $data_dir ]]; then
    fail "No data directory"
fi

# There may be more than one output file of each kind if the test ran across midnight UTC; that's OK.

n=$(cat $data_dir/*/*/*/0+sysinfo*json | wc -c)
if (( n == 0 )); then
    fail "Sysinfo file should not be empty"
fi

n=$(cat $data_dir/*/*/*/0+sample*json | wc -c)
if (( n == 0 )); then
    fail "Sample file should not be empty"
fi

echo " Ok"

