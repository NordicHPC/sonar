#!/bin/bash
#
# Usage:
#  run_test.sh path-to-sonar-binary
#
# This tests the accumulation of cpu time to nested jobs.
#
# A job is a Unix "process group".  Within a process group, the cpu time of children accrue to the
# process group leader as the children terminate (are waited for).  But when a process in a group
# forks off a new process group - creates a subjob - the cpu time of the subjob should not accrue
# the the parent job.
#
# To test this:
#
#  - this script runs `sonar-job N` where N >= 0
#  - if N > 0, that program forks off `sonar-job N-1`
#  - each job is its own process group
#  - each sonar-job then runs some copies of sonar-work, the cpu time of which will accrue to the job
#  - sonar runs in the background to observe the goings-on
#  - in the end, when the topmost sonar-job returns, we want to check that each sonar-job has gotten
#    cpu time accrued to it corresponding to the sum of of its workers, not including subjobs, and
#    also that this script has not been charged for the work of any of the jobs.
#  - this script is called run_test.sh and this is known to the awk processing script, q.v.
#
# This script must be run from the directory that has sonar-job.c, sonar-worker.cc, Makefile and
# check-output.awk.

set -e
if (( $# != 1 )); then
    echo "Usage: $0 path-to-sonar-binary"
    exit 2
fi
SONARBIN=$1

# Config these, if you must
SLEEPTIME=3
WAITTIME=10
NUMJOBS=2
OUTFILE=sonar-output.txt

# Compile things as necessary
make --quiet

# Run sonar in the background, every few seconds, and try to capture no more than necessary.
rm -f $OUTFILE
( while true ; do
      $SONARBIN ps --batchless --exclude-system-jobs | grep -E ',cmd=(sonar|run_test|bash)' >> $OUTFILE
      sleep $SLEEPTIME
  done ) &
SONARPID=$!

# Fork off a new job tree - with parameter N, we should get N+1 levels
echo "The test will take several minutes"
then=$(date +%s)
./sonar-job $((NUMJOBS - 1))
now=$(date +%s)

# Wait for session time to be accrued to this shell
echo "Waiting $((SLEEPTIME * 2))s in run_test (#1) for things to settle..."
sleep $((SLEEPTIME * 2))
kill -TERM $SONARPID

# New output may arrive late after the script has been killed
echo "Waiting $((SLEEPTIME * 2))s in run_test (#2) for things to settle..."
sleep $((SLEEPTIME * 2))

# Now process the output.
gawk -vNUMJOBS=$NUMJOBS -vWALLTIME=$((now - then)) -f check-output.awk $OUTFILE
echo "Everything is fine"
