#!/usr/bin/env bash
#
# Check the interrupt handling for the daemon mode.

set -e

echo " This takes about 45s"

# The ini file sets a 100s time limit, a 3s recording cadence, and a 20s output delay.  Nothing
# should be printed in the first 20s.  Output goes to stdout except for some diagnostics to stderr.
# What we want here is to test several things:
#
# - the daemon catches signals
# - it knows the signal it catches
# - it flushes buffered output (despite being in the "delay" window)
# - it exits cleanly and does not linger very long
#
# To test this, we send it a signal after 10s.  At this point, nothing should have been printed, but
# approximately 3 lines of output should have been accumulated in the buffer.

mkdir -p tmp
output=tmp/daemon-interrupt-output.txt
log=tmp/daemon-interrupt-log.txt

for signal in TERM INT HUP; do
    echo "  Testing SIG$signal"
    rm -f $output $log

    # Fork off the daemon in the background

    cargo run -- daemon daemon-interrupt.ini > $output 2> $log &
    pid=$!

    # Wait for some output to accumulate in internal buffers

    sleep 10

    # Output should have been held

    if (( $(wc -l < $output) != 0 )); then
        echo "Output file is not empty, output-delay did not work"
        exit 1
    fi

    # Output should be flushed by this

    echo "  Killing $pid with SIG$signal"
    kill -$signal $pid
    sleep 2

    # The process should have exited

    if [[ -n $(ps -h -p $pid) ]]; then
        echo "Daemon failed to stop after 2s following signal"
        exit 1
    fi

    # At this point we should have 3 to 5 lines in daemon-interrupt-output.txt.  It's possible for
    # there to be 5 lines with a 3s cadence: on-startup sampling is enabled; and then we can have a
    # sample at 1/4/7/10 (indeed at 0/3/6/9).  I've seen this in practice.  Maybe on-startup
    # sampling should be disabled to tighten the test.  However, since `cargo run` is sort of slow
    # we can have fewer...

    lines=$(grep '{"topic":' $output | wc -l)
    if (( lines < 2 || lines > 5 )); then
        echo "Output file is too short or too long, flushing did not work or something else is off"
        exit 1
    fi

    # daemon-interrupt-log.txt should have info about the current signal, from stderr
    # But `cargo run` pollutes it, so look only at the last line.

    s="xxx"
    case $signal in
        HUP) s=1 ;;
        INT) s=2 ;;
        TERM) s=15 ;;
    esac
    expect="Info: Received signal $s"
    if [[ $(tail -n 1 $log) != $expect ]]; then
        echo "Incorrect signal information, expected \'$expect\'"
        exit 1
    fi
done
echo " Interrupt handling OK"
