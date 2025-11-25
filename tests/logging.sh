#!/usr/bin/env bash
#
# Check that logging is somewhat sane

source sh-helper
assert cargo

output=$(tmpfile logging)
inifile=$(tmpfile logging-ini)

# Default log timestamps are UTC, ending in "Z"

# First test that the log control works as expected

# The default log level in a debug build is "WARN"
SONARTEST_LOGGING=1 cargo run -- sysinfo > /dev/null 2> $output
if ! grep -q -F 'Z ERROR [sonar' $output; then
    fail "Expected ERROR"
fi
if ! grep -q -F 'Z WARN  [sonar' $output; then
    fail "Expected WARN"
fi
if grep 'Z (INFO |DEBUG|TRACE) \[sonar' $output; then
    fail "Did not expect INFO, DEBUG, or TRACE"
fi

# We can disable all logging
SONARTEST_LOGGING=1 RUST_LOG=off cargo run -- sysinfo > /dev/null 2> $output
if grep 'Z (ERROR |WARN |INFO |DEBUG|TRACE) \[sonar' $output; then
    fail "Did not expect any log output"
fi

# We can enable everything
SONARTEST_LOGGING=1 RUST_LOG=trace cargo run -- sysinfo > /dev/null 2> $output
for i in ERROR 'WARN ' 'INFO ' DEBUG TRACE; do
    if ! grep -q -F "Z $i [sonar" $output ; then
        fail "Expected $i"
    fi
done

# Next test that for daemon, enabling verbose gets us the DEBUG level.

echo " This takes about 10 seconds"

# RUST_LOG will override the verbose setting, but the verbose setting by itself will raise the
# default log level to DEBUG.

cat > $inifile <<EOF
[global]
cluster=hpc.axis-of-eval.org
role=node
topic-prefix=zappa

[debug]
time-limit=5s
verbose=true

[sysinfo]
cadence=2s
EOF

cargo run -- daemon $inifile > /dev/null 2> $output
if ! grep -q -F "Z DEBUG [sonar" $output; then
    fail "Expected DEBUG"
fi

RUST_LOG=info cargo run -- daemon $inifile > /dev/null 2> $output
if grep -F "Z DEBUG [sonar" $output; then
    fail "Did not expect DEBUG"
fi

echo " Ok"
