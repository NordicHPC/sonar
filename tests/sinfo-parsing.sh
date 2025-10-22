#!/usr/bin/env bash
#
# Check that `sonar cluster` produces correct output from a known input.

source sh-helper
assert cargo jq

sonar_output=tmp/sinfo-parsing-sinfo-output.tmp
partitions1=tmp/sinfo-parsing-partitions1.tmp
nodes1=tmp/sinfo-parsing-nodes1.tmp
partitions2=tmp/sinfo-parsing-partitions2.tmp
nodes2=tmp/sinfo-parsing-nodes2.tmp
rm -f $sonar_output $partitions1 $partitions2 $nodes1 $nodes2

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
