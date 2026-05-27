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
                for fakegpu in "" "fakegpu"; do
                    for daemon in "" "daemon"; do
                        features=$(join $amd $nvidia $xpu $habana $daemon $fakegpu)
                        echo "no-defaults with features: $features"
                        cargo run --no-default-features --features="$features" -- sysinfo | jq . > /dev/null
                    done
                done
            done
        done
    done
done

# "no features" is tested above
# daemon with neither kafka or http is tested above
# daemon with both kafka and http is the default and is tested everywhere

echo "daemon with kafka only"
cargo build --no-default-features --features "kafka"

echo "daemon with http only"
cargo build --no-default-features --features "http"

echo " Ok"
