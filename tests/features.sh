#!/usr/bin/env bash
#
# Check that we can compile and run something with various feature sets.

set -e
if [[ -z $(command -v jq) ]]; then
    echo "Install jq first"
    exit 1
fi

echo " Default"
output=$( cargo run -- sysinfo )
jq . <<< $output > /dev/null

join() {
    local xs=$1
    shift 1
    while [[ $1 != "" ]]; do
        xs="$xs,$1"
        shift 1
    done
    echo $xs
}

for amd in "" "amd"; do
    for nvidia in "" "nvidia"; do
        for xpu in "" "xpu"; do
            for habana in "" "habana"; do
                for daemon in "" "daemon"; do
                    features=$(join $amd $nvidia $xpu $habana $daemon)
                    echo "no-defaults with features: $features"
                    output=$( cargo run --no-default-features --features="$features" -- sysinfo )
                    jq . <<< $output > /dev/null
                done
            done
        done
    done
done

echo " OK"
