#!/usr/bin/bash
#
# Scan the Rust dependency tree and collect the licenses and emit them to rust-licenses.txt.  Then grep
# for infective licenses we should not have.

cargo tree --all-features --prefix=none --format='{p}   {l}' | sed 's/ (\*)//g' | sort | uniq > rust-licenses.txt
if grep -i GPL rust-licenses.txt; then
    echo ""
    echo '********************'
    echo "Infective license found, should not have that"
    exit 1
fi
