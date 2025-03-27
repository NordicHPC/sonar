#!/usr/bin/env bash
#
# Primitive test runner, to run locally and on CI.

set -e

# sysinfo-syntax.sh requires jq
# check whether jq is available and exit if not
if ! command -v jq &> /dev/null; then
    echo "ERROR: jq is required for sysinfo-syntax.sh"
    exit 1
fi

# keep tests alphabetical
# later we could just iterate over all scripts that end with .sh
# and are not this script
for test in amd-gpu \
                cluster-no-sinfo \
                cluster-syntax \
                command-line \
                exclude-commands \
                exclude-system-jobs \
                exclude-users \
                features \
                gpuinfo \
                hostname \
                interrupt \
                load \
                lockfile \
                min-cpu-time \
                no-gpu \
                nvidia-gpu \
                ps-syntax \
                rollup \
                rollup2 \
                slurm-no-sacct \
                slurm-syntax \
                sysinfo-syntax \
                user \
            ; do
    echo $test
    ./$test.sh
done

echo "No errors"
