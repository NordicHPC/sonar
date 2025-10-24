#!/usr/bin/env bash
#
# Check that `sonar cluster` produces correct output from a known input.

source sh-helper
assert cargo jq

sonar_output=$(tmpfile sinfo-parsing-sinfo-output)
partitions1=$(tmpfile sinfo-parsing-partitions1)
nodes1=$(tmpfile sinfo-parsing-nodes1)
partitions2=$(tmpfile sinfo-parsing-partitions2)
nodes2=$(tmpfile sinfo-parsing-nodes2)

# This is pretty harsh: we require bitwise-identical output.  The assumption is that we have
# hand-checked the output in testdata.  An alternative is to descend into the produced data here
# with jq and make sure specific values are as expected.  But it amounts to the same thing.

SONARTEST_MOCK_PARTITIONS=testdata/partition_output.txt SONARTEST_MOCK_NODES=testdata/node_output.txt \
			 cargo run -- cluster --json --cluster fox.educloud.no > $sonar_output

# Strip the envelope because it contains a timestamp for when the data were generated.
jq .data.attributes.partitions < testdata/sonar_sinfo_output.txt > $partitions1
jq .data.attributes.partitions < $sonar_output > $partitions2
jq .data.attributes.nodes < testdata/sonar_sinfo_output.txt > $nodes1
jq .data.attributes.nodes < $sonar_output > $nodes2
if ! cmp $partitions1 $partitions2; then
    fail "Sonar partitions differ"
fi
if ! cmp $nodes1 $nodes2; then
    fail "Sonar partitions differ"
fi
echo " Ok"
