#!/usr/bin/env bash
#
# Read slurm data from file and check various things.

source sh-helper
assert cargo jq

echo "This test takes about 10s"

logfile=$(tmpfile daemon-slurm-log)
inifile=$(tmpfile daemon-slurm-ini)

cat > $inifile <<EOF
[global]
cluster=hpc.yes.no
role=node
domain=.yes.no

[debug]
time-limit = 10s

[cluster]
cadence=5s
EOF

SONARTEST_MOCK_PARTITIONS=testdata/partition_output.txt \
    SONARTEST_MOCK_NODES=testdata/node_output.txt \
    cargo run -- daemon $inifile > $logfile

# Check that the domain is added to slurm host sets for partitions and nodes.

n1=$(jq -r '.value.data.attributes.partitions[0].nodes[0]' $logfile | head -n 1)
x1="c1-[5-28].yes.no"
if [[ $n1 != "$x1" ]]; then
    fail "Bad hosts.  Got <$n1> expected <$x1>"
fi

n2=$(jq -c -r '.value.data.attributes.nodes[0].names' $logfile | head -n 1)
x2='["bigmem-1.yes.no","c1-[5,9,12-18].yes.no","gpu-[1,5-6,8-13,15-16].yes.no"]'
if [[ $n2 != "$x2" ]]; then
    fail "Bad hosts.  Got <$n2> expected <$x2>"
fi

echo " Ok"
