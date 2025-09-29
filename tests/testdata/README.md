# Regenerating and using test data

## sacct data

To regenerate the sacct data, we basically want to do this on Fox
(remember to load Rust and Go modules first):

```
SONARTEST_SUBCOMMAND_OUTPUT=sacct_output.txt cargo run -- slurm --deluge --json --cluster fox.educloud.no > sonar_sacct_output.txt
go run anonymize-sacct-data.go -o
```

Now Sonar can be run with SONARTEST_MOCK_SACCT=sacct_output.txt.  See ../sacct-parsing.sh.

## sinfo / cluster data

To regenerate the sinfo data, we basically want to do this on Fox
(remember to load Rust first):

```
SONARTEST_SUBCOMMAND_OUTPUT=partition_output.txt cargo run -- cluster --json --cluster fox.educloud.no > sonar_sinfo_output.txt
mv partition_output.txt.1 node_output.txt
rm partition_output.txt.2
```

Now Sonar can be run with SONARTEST_MOCK_PARTITIONS=partition_output.txt SONARTEST_MOCK_NODES=node_output.txt.  See ../sinfo-parsing.sh.

