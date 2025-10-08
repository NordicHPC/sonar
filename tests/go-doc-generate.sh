#!/bin/bash
#
# Test that all generated docs and code are in sync with the source.
#
# Note, this is not run by `run_tests.sh` since it is normal for docs not to be regenerated during
# development. (On the other hand, it would be desirable for json_tags.rs to be checked during
# development.)

set -e
cd ../util/process-doc
make all 2>&1 > /dev/null
cmp NEW-FORMAT.md ../../doc/NEW-FORMAT.md
cmp json_tags.rs ../../src/json_tags.rs
cmp types.spec.yaml ../../doc/types.spec.yaml

