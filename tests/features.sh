#!/usr/bin/env bash
#
# Check that we can compile and run something with various feature sets.

set -e
if [[ $(command -v jq) == "" ]]; then
    echo "Install jq first"
    exit 1
fi

echo " Default"
( cd .. ; cargo build )
output=$(../target/debug/sonar sysinfo)
jq . <<< $output > /dev/null

for d in "" ",daemon"; do
    echo " no-defaults$d"
    ( cd .. ; cargo build --no-default-features --features "$d" )
    output=$(../target/debug/sonar sysinfo)
    jq . <<< $output > /dev/null

    echo " amd$d"
    ( cd .. ; cargo build --no-default-features --features amd$d )
    output=$(../target/debug/sonar sysinfo)
    jq . <<< $output > /dev/null

    echo " nvidia$d"
    ( cd .. ; cargo build --no-default-features --features nvidia$d )
    output=$(../target/debug/sonar sysinfo)
    jq . <<< $output > /dev/null

    echo " nvidia,amd$d"
    ( cd .. ; cargo build --no-default-features --features nvidia,amd$d )
    output=$(../target/debug/sonar sysinfo)
    jq . <<< $output > /dev/null
done

# No XPU library yet so this feature should cause link failure

echo " XPU"
if [[ $( cd .. ; cargo build --no-default-features --features xpu 2> /dev/null ) ]]; then
    echo "XPU test should have failed but did not"
    exit 1
fi

echo " OK"
