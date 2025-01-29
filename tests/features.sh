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

echo " None"
( cd .. ; cargo build --no-default-features )
output=$(../target/debug/sonar sysinfo)
jq . <<< $output > /dev/null

echo " AMD"
( cd .. ; cargo build --no-default-features --features amd )
output=$(../target/debug/sonar sysinfo)
jq . <<< $output > /dev/null

echo " NVIDIA"
( cd .. ; cargo build --no-default-features --features nvidia )
output=$(../target/debug/sonar sysinfo)
jq . <<< $output > /dev/null

echo " NVIDIA,AMD"
( cd .. ; cargo build --no-default-features --features nvidia,amd )
output=$(../target/debug/sonar sysinfo)
jq . <<< $output > /dev/null

# No XPU library yet so this feature should cause link failure

echo " XPU"
if [[ $( cd .. ; cargo build --no-default-features --features xpu 2> /dev/null ) ]]; then
    echo "XPU test should have failed but did not"
    exit 1
fi

echo " OK"
