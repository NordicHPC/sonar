#!/bin/bash
#
# This will run sonar against a local kafka broker with some interesting properties.  See README.md
# for more information.

SCRIPT_DIR=$(realpath -L $(dirname $0))

( cd $SCRIPT_DIR/../.. ; cargo build )

# This shoule not be necessary if you followed the instruction in README.md
# ( cd ./ssl ; make all )

$SCRIPT_DIR/../../../target/debug/sonar daemon sonar-nonslurm-node-ssl-sasl.ini
