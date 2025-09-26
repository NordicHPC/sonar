#!/usr/bin/env bash
#
# Primitive test runner, to run locally and on CI.
#
# To start with a particular test, pass its name as the first arg.

set -e

# sysinfo-syntax.sh requires jq
# check whether jq is available and exit if not
if ! command -v jq &> /dev/null; then
    echo "ERROR: jq is required for sysinfo-syntax.sh"
    exit 1
fi

# keep tests alphabetical
# later we could just iterate over all scripts that end with .sh
tests="amd-gpu \
     cluster-no-sinfo \
     cluster-syntax \
     command-line \
     daemon \
     daemon-directory \
     daemon-interrupt \
     daemon-kafka \
     exclude-commands \
     exclude-system-jobs \
     exclude-users \
     features \
     gpuinfo \
     habana-gpu \
     hostname \
     load \
     lockfile \
     min-cpu-time \
     no-gpu \
     nvidia-gpu \
     ps-interrupt \
     ps-syntax \
     rollup \
     rollup2 \
     regress-369-kafka-pump \
     slurm-no-sacct \
     slurm-syntax \
     sysinfo-syntax \
     sysinfo-topo \
     user \
     xpu-gpu"

running=0
if [[ -z $1 ]]; then
    running=1
fi
for test in $tests; do
    echo $test
    if [[ $running == 0 && $test == $1 ]]; then
        running=1
    fi
    if [[ $running == 0 ]]; then
        continue
    fi
    ./$test.sh
done

echo "No errors"
