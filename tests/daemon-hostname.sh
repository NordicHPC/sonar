#!/usr/bin/env bash
#
# Check that the daemon produces the correct hostnames.  This is analogous to hostname.sh but for
# the daemon, it checks that the setting is propagated correctly.

source sh-helper
assert cargo

datafile=$(tmpfile daemon-hostname-data)
logfile=$(tmpfile daemon-hostname-log)
inifile=$(tmpfile daemon-hostname-ini)

for opt in "" hostname-only=true; do
    for cmd in sample sysinfo jobs cluster; do
        if [[ ($cmd == "jobs" || $cmd == "cluster") && -z $(command -v sinfo) ]]; then
            continue
        fi

        cat > $inifile <<EOF
[global]
cluster=hpc.axis-of-eval.org
role=node
$opt

[debug]
verbose=true
oneshot=true

[$cmd]
cadence=1s
EOF

        cargo run -- daemon $inifile 2>$logfile >$datafile
        case $cmd in
            sample | sysinfo)
                if [[ -z $opt ]]; then
                    expect=$(hostname)
                else
                    expect=$(hostname | grep -o -E '^[a-zA-Z0-9-]+')
                fi
                node=$(jq --raw-output '.value.data.attributes.node' $datafile)
                if [[ $node != $expect ]]; then
                    fail "$cmd - Wrong hostname?  Got $node expected $expect"
                else
                    echo " $cmd $opt ok"
                fi
                ;;
            cluster)
                # This asserts that there will be naked hostnames even without hostname-only=true
                # and the test may start failing when we fix #459.
                if jq '.value.data.attributes.partitions[].nodes[]' $datafile | grep -F '.'; then
                    fail "cluster produced output with dots in partitions"
                else
                    echo " cluster $opt partitions ok"
                fi
                if jq '.value.data.attributes.nodes[].names[]' $datafile | grep -F '.'; then
                    fail "cluster produced output with dots in nodes"
                else
                    echo " cluster $opt nodes ok"
                fi
                ;;
            jobs)
                # This asserts that there will be naked hostnames even without hostname-only=true
                # and the test may start failing when we fix #459.
                if jq -r '.value.data.attributes.slurm_jobs[].nodes | select(. != null) | .[]' $datafile | grep -F '.'; then
                    fail "jobs produced output with dots"
                else
                    echo " jobs $opt ok"
                fi
                ;;
        esac
    done
done

echo " Ok"

