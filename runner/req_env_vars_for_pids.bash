#!/usr/bin/env bash
#
# NOTE!  This must be run interactively from a shell because it'll ask for a root password and
# cannot be started as root.
#
# Selftest code for the REQ_ENV_VARS_FOR_PIDS function of the sonar-runner.  Stage 1.  Run this as a
# regular user, it will sudo to run stage 2, which will run sonar-mock or sonar after dropping
# privileges back to this user.
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

make
( cd .. ; cargo build )

# I guess to keep this test case completely separated from the other one we should use estragon
# here, not vladimir.
#trap 'pkill vladimir; rm -f test_ev4p_*.txt' EXIT

$(pwd)/build/vladimir &
$(pwd)/build/vladimir &
$(pwd)/build/vladimir &
join $(pgrep vladimir) > test_ev4p_pids.txt
echo $(whoami) $(groups | awk '{ print $1 }') > test_ev4p_id.txt

echo "Testing with sonar-mock"
echo ./build/sonar-mock > test_ev4p_exe.txt
sudo ./req_env_vars_for_pids_stage2.bash
#cat test_ev4p_result.txt
count=0
for p in $(pgrep vladimir) ; do
    # There should always be some
    grep --text "pid=$p" test_ev4p_result.txt > test_ev4p_candidates.txt
    numcand=$(cat test_ev4p_candidates.txt | wc -l)
    # Every one should have a variable with PATH in the name b/c that's what the
    # test code in sonar-mock asks for.
    numpath=$(grep --text 'name=.*PATH.* value=' test_ev4p_candidates.txt | wc -l)
    if ((numcand != numpath)); then
        echo "Inconsistent result for $p: $numcand $numpath"
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
echo ../target/debug/sonar > test_ev4p_exe.txt
sudo ./req_env_vars_for_pids_stage2.bash

cat test_ev4p_result.txt

# In this case, test_ev4p_result.txt should be raw JSON output
# jq -c -r '.value.data.attributes.jobs[].processes[] | {"exe":.exe, "pid":.pid}' < test_ev4p_result.txt | grep vladimir > test_ev4p_results2.txt
# #cat test_ev4p_results2.txt

# count=0
# for p in $(pgrep vladimir) ; do
#     look="{\"exe\":\"$(pwd)/build/vladimir\",\"pid\":$p}"
#     if ! $(grep -q "$look" test_ev4p_results2.txt); then
#         echo "Did not find $look"
#         exit 1
#     fi
#     count=$((count+1))
# done
# if ((count != 3)); then
#     echo "Bad count: " $count
#     exit 1
# fi
echo "Testing with sonar OK"

echo "All OK"
