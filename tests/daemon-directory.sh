#!/usr/bin/env bash
#
# Collect data into a directory for 20s and then check that data were collected properly.
#
# TODO: We should extend this to check that timestamps and file names are correct.

set -e
echo "This test takes about 20s"
( cd .. ; cargo build )

data_dir=daemon-directory-data
logfile=daemon-directory-log.txt
rm -rf $data_dir $logfile
../target/debug/sonar daemon daemon-directory.ini 2>$logfile

if [[ ! -d $data_dir ]]; then
    echo "No data directory"
    exit 1
fi

# There may be more than one output file of each kind if the test ran across midnight UTC; that's OK.

n=$(cat $data_dir/*/*/*/0+sysinfo*json | wc -c)
if (( $n == 0 )); then
    echo "Sysinfo file should not be empty"
    exit 1
fi

n=$(cat $data_dir/*/*/*/0+sample*json | wc -c)
if (( $n == 0 )); then
    echo "Sample file should not be empty"
    exit 1
fi

echo " OK"

