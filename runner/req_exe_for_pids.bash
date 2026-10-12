#!/usr/bin/env bash
#
# NOTE!  This must be run interactively from a shell because it'll ask for a root password and
# cannot be started as root.
#
# Selftest code for the REQ_EXE_FOR_PIDS function of the sonar-runner.  Stage 1.  Run this as a
# regular user, it will sudo to run stage 2, which will run the sonar-mock after dropping privileges
# back to this user.
#
# The expected result here is that this runs for a bit and then exits with the "All OK" message.

# Does not work because exit status of sonar-runner becomes propagated somehow
# set -e

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
( cd .. ; cargo build )

$(pwd)/vladimir &
$(pwd)/vladimir &
$(pwd)/vladimir &
join $(pgrep vladimir) > test_pids.txt
echo $(whoami) $(groups | awk '{ print $1 }') > test_id.txt

echo "Testing with sonar-mock"
echo ./sonar-mock > test_exe.txt
sudo ./req_exe_for_pids_stage2.bash
count=0
for p in $(pgrep vladimir) ; do
    look="pid=$p path=$(pwd)/vladimir"
    if ! $(grep -q "$look" test_result.txt); then
        echo "Did not find $look"
        pkill vladimir
        exit 1
    fi
    count=$((count+1))
done
if ((count != 3)); then
    echo "Bad count: " $count
    exit 1
fi
echo "Testing with sonar-mock OK"

echo "Testing with sonar"
echo ../target/debug/sonar > test_exe.txt
sudo ./req_exe_for_pids_stage2.bash
# In this case, test_result.json should be raw JSON output
jq -c -r '.value.data.attributes.jobs[].processes[] | {"exe":.exe, "pid":.pid}' < test_result.txt | grep vladimir > test_results2.txt
#cat test_results2.txt

count=0
for p in $(pgrep vladimir) ; do
    look="{\"exe\":\"$(pwd)/vladimir\",\"pid\":$p}"
    if ! $(grep -q "$look" test_results2.txt); then
        echo "Did not find $look"
        pkill vladimir
        exit 1
    fi
    count=$((count+1))
done
if ((count != 3)); then
    echo "Bad count: " $count
    exit 1
fi
echo "Testing with sonar OK"

pkill vladimir
rm -f test_*.txt
echo "All OK"
