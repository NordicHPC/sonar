#!/bin/bash
#
# Usage: run_test.sh path-to-sonar-binary
#
# This must be run from the directory with sonar-session-root and sonar-job-root and sonar-worker

if (( $# != 1 )); then
    echo "Usage: $0 path-to-sonar-binary"
    exit 2
fi

SLEEPTIME=5
WAITTIME=10

SONARBIN=$1
OUTFILE=sonar-output.$$.txt

# Compile things as necessary
make

# Run sonar in the background, every few seconds, and try to capture no more than necessary.
rm -f $OUTFILE
( while true ; do
      $SONARBIN ps | egrep ",user=$LOGNAME," | egrep 'sonar|bash' >> $OUTFILE
      sleep $SLEEPTIME
  done ) &
SONARPID=$!

# Run the new session root
./sonar-session-root

# Wait for everything to stabilize, then stop sonar
sleep $WAITTIME
kill -TERM $SONARPID

# Now process the output.
