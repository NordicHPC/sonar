#!/usr/bin/env bash
#
# Check that `sonar ps` produces some sane output.

source sh-helper
assert cargo jq

# JSON
#
# For the JSON output we can use jq.

# Check that it is syntactically sane

output=$(cargo run -- ps --load --exclude-system-jobs --cluster x --json)
jq . <<< $output > /dev/null

# Check that the envelope has required fields
version=$(jq .meta.version <<< $output)
if [[ !( $version =~ [0-9]+\.[0-9]+\.[0-9](-.+)? ) ]]; then
    fail "JSON version bad, got $version"
fi
x=$(jq .meta.producer <<< $output)
if [[ $x != '"sonar"' ]]; then
    fail "JSON producer wrong, got $x"
fi
# Check that the type is right
x=$(jq .data.type <<< $output)
if [[ $x != '"sample"' ]]; then
    fail "JSON type wrong, got $x"
fi
# Check that data envelope has some required fields
x=$(jq .data.attributes.time <<< $output)
if [[ $x == "null" ]]; then
    fail "JSON time missing"
fi
x=$(jq .data.attributes.node <<< $output)
if [[ $x == "null" ]]; then
    fail "JSON node missing"
fi
x=$(jq .data.attributes.jobs <<< $output)
if [[ $x == "null" ]]; then
    fail "JSON jobs missing"
fi
x=$(jq .data.attributes.system <<< $output)
if [[ $x == "null" ]]; then
    fail "JSON system missing"
fi

# If there's at least one sample, check that it has at least user and cmd, which are required.

first=$(jq '.data.attributes.jobs[0]' <<< $output)
if [[ $first != "null" ]]; then
    user=$(jq .user <<< $first)
    if [[ $user == "null" ]]; then
        fail "JSON user missing"
    fi
    cmd=$(jq .processes[0].cmd <<< $first)
    if [[ $cmd == "null" ]]; then
        fail "JSON cmd missing"
    fi
fi

# Since we specified --load there should be various fields in the system object.

# cpus should really never be absent
x=$(jq .data.attributes.system.cpus <<< $output)
if [[ $x == "null" ]]; then
    fail "JSON system.cpus missing"
fi

# The others can be zero/empty and thus absent.  What we need to check here is that
# every field present has a known name.

x=$(jq '.data.attributes.system | keys | map(in({"boot":0,"cpus":0,"gpus":0,"disks":0,"existing_entities":0,"load1":0,"load15":0,"load5":0,"runnable_entities":0,"used_memory":0})) | all' <<< $output)
if [[ $x != "true" ]]; then
    fail "JSON bad - Unknown field in system:" $(jq '.data.attributes.system | keys' <<< $output)
fi

echo " Ok: JSON"

