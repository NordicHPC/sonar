#!/usr/bin/env bash

source sh-helper
guard docker
guard_group docker

output=$(tmpfile docker-out)
selected=$(tmpfile docker-selected)

echo " The test takes about 20s and may download a docker image"
make pincpu pincpus
cargo build

# Single-process test.  The pincpu program should show up as being owned by root.

docker run -d -v .:/sonar:z --rm -it ubuntu /sonar/pincpu 10

sleep 5
cargo run -- ps --exclude-system-jobs > $output

jq -c '.data.attributes.jobs[] | {user, process: .processes[]} | select(.user == "root") | [.user, .process.cmd, .process.in_container]' $output > $selected
if ! grep -q -F '["root","pincpu",true]' $selected; then
    cat $selected
    fail "Did not find pincpu process"
fi

# Let's hope this is enough to make the first one terminate

sleep 5

# Multi-process / process tree test.  The pincpus program runs in a docker, it forks off two pincpu
# children with the same argument and then waits for them.  We should see all three in the docker
# output as being marked as running in a container, and the children should have the parent as ppid.

docker run -d -v .:/sonar:z --rm -it ubuntu /sonar/pincpus /sonar/pincpu 2 10

sleep 5
cargo run -- ps --exclude-system-jobs > $output

# TODO: count two pincpu processes (assuming there's nobody else on the system)
# TODO: pincpu processes should have pincpus process as parent

jq -c '.data.attributes.jobs[] | {user, process: .processes[]} | select(.user == "root") | [.user, .process.cmd, .process.in_container]' $output > $selected
if ! grep -q -F '["root","pincpu",true]' $selected; then
    cat $selected
    fail "Did not find pincpu process"
fi
if ! grep -q -F '["root","pincpus",true]' $selected; then
    cat $selected
    fail "Did not find pincpus process"
fi

echo " Ok"
