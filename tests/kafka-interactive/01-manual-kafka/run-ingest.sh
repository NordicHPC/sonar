#!/bin/bash

( cd ../../../util/ingest-kafka ; go build )
mkdir -p test-cluster
../../../util/ingest-kafka/ingest-kafka -cluster test-cluster.hpc.uio.no -data-dir test-cluster -broker localhost:9099 -v

