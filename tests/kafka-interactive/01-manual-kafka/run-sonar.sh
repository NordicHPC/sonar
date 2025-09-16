#!/bin/bash
#
# This will run sonar against a local kafka broker with some interesting properties.  See README.md
# for more information.

( cd ../../.. ; cargo build )
( cd ../../../util/ssl ; make all )
( cd ../../.. ; cargo run -- daemon sonar-nonslurm-node-ssl-saslfile.ini )

