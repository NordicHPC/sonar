#!/usr/bin/env bash
#
# Collect data into a directory for 20s and then check that data were collected properly.
#
# TODO: We should extend this to check that timestamps and file names are correct.

source sh-helper
assert cargo

echo "This test takes about 20s"

data_dir=$(tmpdir daemon-directory-data)
logfile=$(tmpfile daemon-directory-log)
inifile=$(tmpfile daemon-directory-ini)

cat > $inifile <<EOF
[global]
cluster=hpc.axis-of-eval.org
role=node

[debug]
verbose = true
# Set the time limit to 20s so that we'll terminate after a few samples, this is to aid
# automated testing.
time-limit = 20s

[directory]
data-directory = $data_dir

[sample]
cadence=4s

[sysinfo]
cadence=5s
EOF

cargo run -- daemon $inifile 2>$logfile
if [[ ! -d $data_dir ]]; then
    fail "No data directory $data_dir"
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

