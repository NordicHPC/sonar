#!/usr/bin/env bash
#
# Check that the Kafka HTTP data sink does its job.  In this case, we use a script to echo back the
# arguments and output that Sonar would otherwise hand to curl, and we check that that output looks
# right.

source sh-helper
assert cargo jq

echo "This test takes about 10s"

outfile=$(tmpfile daemon-http-output)
logfile=$(tmpfile daemon-http-log)
inifile=$(tmpfile daemon-http-ini)
curly=$(tmpfile daemon-http-curly)
curly_out=$(tmpfile daemon-http-curly-out)
msg_data=$(tmpfile daemon-http-curly-msg)

cat > $curly <<EOF
#!/bin/bash
echo "Args: \$@" >> $curly_out
cat >> $curly_out
EOF
chmod a+x $curly

cat > $inifile <<EOF
[global]
cluster=hpc.axis-of-eval.org
role=node

[programs]
curl-command = $(pwd)/tmp/daemon-http-curly.tmp

[debug]
verbose = true
time-limit = 5s

[kafka]
rest-endpoint = no.such.host:101010
sending-window = 3s
sasl-password = foobar

[sysinfo]
cadence=1s
EOF

rm -f $curly_out
cargo run -- daemon $inifile > $outfile 2> $logfile

if grep -q -F 'ERROR [sonar' $logfile; then
    fail "Errors in log file"
fi

if [[ -s $outfile ]]; then
    fail "Output file is not empty"
fi

if ! grep -q '^Args:' $curly_out; then
    fail "curly output does not have Args lines"
fi

if ! grep '^{' $curly_out > $msg_data; then
    fail "no message data"
fi

# The first line of a pair should have topic etc
# The second line of a pair should have a sysinfo object
# Let's just look at the first pair

control=$(head -n1 $msg_data)
data=$(head -n2 $msg_data | tail -n1)
if [[ $(echo $control | jq -r .topic) != hpc.axis-of-eval.org.sysinfo ]]; then
    fail "Control object likely wrong" $control
fi
if [[ $(echo $data | jq -r .data.attributes.cluster) != hpc.axis-of-eval.org ]]; then
    fail "Data object likely wrong" $data
fi

echo " Ok"



