#!/usr/bin/env bash
#
# Check that `sonar ps` produces some sane output.

set -e
if [[ -z $(command -v jq) ]]; then
    echo "Install jq first"
    exit 1
fi

# CSV
#
# At the moment, all we do for CSV is:
#  - check that at least one line is produced
#  - the line starts with `v=` with a sensible version syntax
#  - there is a `host=$HOSTNAME` field
#  - there is a `user=` field with a plausible string (note bug 192, may contain "-")
#  - there is a `cmd=` field
#
# We don't have the infra at the moment to check the CSV output, plus CSV is so flexible that it's
# sort of hard to check it.

output=$(cargo run -- ps)
count=$(wc -l <<< $output)
if (( count <= 0 )); then
    echo "Must have some number of output lines"
    exit 1
fi
l=$(head -n 1 <<< $output)
if [[ !( $l =~ ^v=[0-9]+\.[0-9]+\.[0-9]+(-.+)?, ) ]]; then
    echo "CSV version missing, got $l"
    exit 1
fi
if [[ !( $l =~ ,user=[-a-z0-9_]+, ) ]]; then
    echo "CSV user missing, got $l"
    exit 1
fi
# The command may be quoted so match only the beginning
if [[ !( $l =~ ,\"?cmd= ) ]]; then
    echo "CSV cmd missing, got $l"
    exit 1
fi
if [[ !( $l =~ ,host=$HOSTNAME, ) ]]; then
    echo "CSV host missing, got $l"
    exit 1
fi

echo " Ok: CSV"

# JSON
#
# For the JSON output we can use jq.

# Check that it is syntactically sane

output=$(cargo run -- ps --load --exclude-system-jobs --cluster x --json)
jq . <<< $output > /dev/null

# Check that the envelope has required fields
version=$(jq .meta.version <<< $output)
if [[ !( $version =~ [0-9]+\.[0-9]+\.[0-9](-.+)? ) ]]; then
    echo "JSON version bad, got $version"
    exit 1
fi
x=$(jq .meta.producer <<< $output)
if [[ $x != '"sonar"' ]]; then
    echo "JSON producer wrong, got $x"
    exit 1
fi
# Check that the type is right
x=$(jq .data.type <<< $output)
if [[ $x != '"sample"' ]]; then
    echo "JSON type wrong, got $x"
    exit 1
fi
# Check that data envelope has some required fields
x=$(jq .data.attributes.time <<< $output)
if [[ $x == "null" ]]; then
    echo "JSON time missing"
    exit 1
fi
x=$(jq .data.attributes.node <<< $output)
if [[ $x == "null" ]]; then
    echo "JSON node missing"
    exit 1
fi
x=$(jq .data.attributes.jobs <<< $output)
if [[ $x == "null" ]]; then
    echo "JSON jobs missing"
    exit 1
fi
x=$(jq .data.attributes.system <<< $output)
if [[ $x == "null" ]]; then
    echo "JSON system missing"
    exit 1
fi

# If there's at least one sample, check that it has at least user and cmd, which are required.

first=$(jq '.data.attributes.jobs[0]' <<< $output)
if [[ $first != "null" ]]; then
    user=$(jq .user <<< $first)
    if [[ $user == "null" ]]; then
        echo "JSON user missing"
        exit 1
    fi
    cmd=$(jq .processes[0].cmd <<< $first)
    if [[ $cmd == "null" ]]; then
        echo "JSON cmd missing"
        exit 1
    fi
fi

# Since we specified --load there should be various fields in the system object.

# cpus should really never be absent
x=$(jq .data.attributes.system.cpus <<< $output)
if [[ $x == "null" ]]; then
    echo "JSON system.cpus missing"
    exit 1
fi

# The others can be zero/empty and thus absent.  What we need to check here is that
# every field present has a known name.

x=$(jq '.data.attributes.system | keys | map(in({"cpus":0,"gpus":0,"existing_entities":0,"load1":0,"load15":0,"load5":0,"runnable_entities":0,"used_memory":0})) | all' <<< $output)
if [[ $x != "true" ]]; then
    echo "JSON bad - Unknown field in system: " $(jq .data.attributes.system <<< $output)
    exit 1
fi

echo " Ok: JSON"

