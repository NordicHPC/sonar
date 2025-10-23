#!/usr/bin/env bash
#
# Check that we can compile and run something with various feature sets.

source sh-helper
assert cargo jq

echo " Default"
cargo run -- sysinfo | jq . > /dev/null

for amd in "" "amd"; do
    for nvidia in "" "nvidia"; do
        for xpu in "" "xpu"; do
            for habana in "" "habana"; do
                for daemon in "" "daemon"; do
                    features=$(join $amd $nvidia $xpu $habana $daemon)
                    echo "no-defaults with features: $features"
                    cargo run --no-default-features --features="$features" -- sysinfo | jq . > /dev/null
                done
            done
        done
    done
done

echo " Ok"
