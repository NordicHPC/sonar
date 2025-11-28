#!/usr/bin/env bash

source sh-helper
guard docker
guard_group docker

output=$(tmpfile docker)

echo " The test takes about 10s and may download a docker image"
make pincpu
cargo build
docker run -d -v .:/sonar:z --rm -it ubuntu /sonar/pincpu 10

sleep 5
cargo run -- ps --exclude-system-jobs > $output

# TODO:
#  - This is brittle: if there are other `docker run` jobs running on the system they will also
#    show up here.
#  - Process trees, for sure

pincpu=$(jq -c '.data.attributes.jobs[] | {user, process: .processes[]} | select(.user == "root") | [.user, .process.cmd, .process.in_container]' $output)
if [[ $pincpu != '["root","pincpu",true]' ]]; then
    fail "Did not find pincpu process: $pincpu"
fi

echo " Ok"
