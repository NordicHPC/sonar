#!/usr/bin/env bash
#
# Run the rollup test in repeat daemon mode, this will synthesize PIDs for rolled-up jobs but as
# jobs come and go it will run GC and reuse the PIDs.  Also see daemon-rollup.sh.
#
# For now this just tests that things don't crash (although it's a useful test to run interactively
# with logging to the console).  It's hard to set up a test case that runs the same on all systems,
# as some systems have processes that are seen by Sonar that will be rolled up and will pollute the
# results.

source sh-helper
assert cargo cc

output=$(tmpfile daemon-rollup2-out)
log=$(tmpfile daemon-rollup2-log)
inifile=$(tmpfile daemon-rollup2-ini)

make rollup-programs

echo " This takes about 60s"

cat > $inifile <<EOF
[global]
cluster = example.com
role = node

[debug]
time-limit = 60s
verbose = true

[sample]
cadence = 3s
exclude-system-jobs = true
load = false
rollup = true
EOF

(
    ./daemon-rollup2 &

    # pid pool size = 12, min collected range = 2
    RUST_BACKTRACE=1 SONARTEST_ROLLUP=1 SONARTEST_ROLLUP_PIDS=12,2 cargo run -- daemon $inifile > $output
) 2> $log

echo " Ok"
