#!/bin/bash
#
# Test that all generated docs and code are in sync with the source.

set -e
cd ../../util/process-doc
make all 2>&1 > /dev/null
cmp NEW-FORMAT.md ../../doc/NEW-FORMAT.md
cmp json_tags.rs ../../src/json_tags.rs
cmp types.spec.yaml ../../doc/types.spec.yaml

