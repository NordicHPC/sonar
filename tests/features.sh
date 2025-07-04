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

    echo " amd,xpu$d"
    ( cd .. ; cargo build --no-default-features --features amd,xpu$d )
    output=$(../target/debug/sonar sysinfo)
    jq . <<< $output > /dev/null

    echo " nvidia,xpu$d"
    ( cd .. ; cargo build --no-default-features --features nvidia,xpu$d )
    output=$(../target/debug/sonar sysinfo)
    jq . <<< $output > /dev/null

    echo " nvidia,amd,xpu$d"
    ( cd .. ; cargo build --no-default-features --features nvidia,amd,xpu$d )
    output=$(../target/debug/sonar sysinfo)
    jq . <<< $output > /dev/null
done

# No Habana library yet so this feature should cause link failure

echo " HABANA"
if [[ $( cd .. ; cargo build --no-default-features --features habana 2> /dev/null ) ]]; then
    echo "Habana test should have failed but did not"
    exit 1
fi

echo " OK"
