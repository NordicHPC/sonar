#!/bin/bash
#
# Usage:
#  run_test.sh path-to-sonar-binary
#
# This must be run from the directory that has sonar-session.c, sonar-job.c, sonar-worker.cc,
# Makefile and check-output.awk.

set -e
if (( $# != 1 )); then
    echo "Usage: $0 path-to-sonar-binary"
    exit 2
fi
SONARBIN=$1

# Config these, if you must
SLEEPTIME=3
WAITTIME=10
OUTFILE=sonar-output.txt

# Compile things as necessary
make --quiet

# Run sonar in the background, every few seconds, and try to capture no more than necessary.
rm -f $OUTFILE
( while true ; do
      $SONARBIN ps | grep -E ",user=$LOGNAME," | grep -E ',cmd=(sonar|bash)' >> $OUTFILE
      sleep $SLEEPTIME
  done ) &
SONARPID=$!

# Run the new session root
echo "The test can take 60-90s on 2024-era hardware"
then=$(date +%s)
./sonar-session
now=$(date +%s)

kill -TERM $SONARPID

# Now process the output.  See check-output.c for details.
gawk -vWALLTIME=$((now - then)) -f check-output.awk $OUTFILE
echo "Everything is fine"
