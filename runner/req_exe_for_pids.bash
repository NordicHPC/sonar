#!/usr/bin/env bash
#
# This must be run interactively from a shell.
#
# Selftest code for the REQ_EXE_FOR_PIDS function of the sonar-runner.  Stage 1.  Run this as a
# regular user, it will sudo to run stage 2, which will run the sonar-mock after dropping privileges
# back to this user.
#
# The expected result here is that this runs for a bit and then exits with the "All OK" message.

if [[ $(whoami) == "root" ]]; then
    echo "Do not run as root, it will sudo"
    exit 1
fi


function join() {
    local xs=$1
    shift 1
    while [[ -n $1 ]]; do
        xs="$xs,$1"
        shift 1
    done
    echo $xs
}

make all

$(pwd)/godot &
$(pwd)/godot &
$(pwd)/godot &
join $(pgrep godot) > test_pids.txt
echo $(whoami) $(groups | awk '{ print $1 }') > test_id.txt
sudo ./req_exe_for_pids_stage2.bash
for p in $(pgrep godot) ; do
    look="pid=$p path=$(pwd)/godot"
    if ! $(grep -q "$look" test_result.txt); then
        echo "Did not find $look"
        pkill godot
        exit 1
    fi
done
pkill godot
rm -f test_pids.txt test_id.txt test_result.txt
echo "All OK"
