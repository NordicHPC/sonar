// `ingest-kafka` will listen for `sample` and `sysinfo` traffic from a local kafka broker, and "do
// something" with the data.  See comments in ../../tests/daemon-local.cfg for an example of how to
// use this.
//
// Currently "do something" is print stuff on stdout, but eventually it may mean placing the data
// in a database.

package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"

	"github.com/NordicHPC/sonar/util/formats/oldfmt"
	"github.com/twmb/franz-go/pkg/kgo"
)

const (
	clusterName = "akebakken.no"
)

var (
	broker = flag.String("broker", "localhost:9092", "Broker `host:port`")
	verbose = flag.Bool("v", false, "Verbose")
)

func main() {
	flag.Parse()
	cl, err := kgo.NewClient(
		kgo.SeedBrokers(*broker),
		kgo.ConsumerGroup("sonar-ingest"),
		kgo.ConsumeTopics(clusterName+".sample", clusterName+".sysinfo"),
	)
	if err != nil {
		panic(err)
	}
	defer cl.Close()
	if *verbose {
		println("Connected")
	}

	ctx := context.Background()

	for {
		if *verbose {
			println("Fetching")
		}
		fetches := cl.PollFetches(ctx)
		if *verbose {
			println("Fetched")
		}
		if errs := fetches.Errors(); len(errs) > 0 {
			// All errors are retried internally when fetching, but non-retriable errors are
			// returned from polls so that users can notice and take action.
			panic(fmt.Sprint(errs))
		}

		iter := fetches.RecordIter()
		for !iter.Done() {
			record := iter.Next()
			switch record.Topic {
			case clusterName + ".sample":
				info := new(oldfmt.SampleEnvelope)
				err := json.Unmarshal(record.Value, info)
				if err != nil {
					panic(err)
				}
				consumeOldSample(info)
			case clusterName + ".sysinfo":
				info := new(oldfmt.SysinfoEnvelope)
				err := json.Unmarshal(record.Value, info)
				if err != nil {
					panic(err)
				}
				consumeOldSysinfo(info)
			default:
				panic("Unknown topic " + record.Topic)
			}
		}
		if err := cl.CommitUncommittedOffsets(ctx); err != nil {
			fmt.Printf("commit records failed: %v", err)
		}
	}
}

func consumeOldSample(info *oldfmt.SampleEnvelope) {
	fmt.Printf("Sample for %s %s\n", info.Hostname, info.Timestamp)
	for _, s := range info.Samples {
		if s.CpuTimeSec > 100 {
			fmt.Printf("  `%s` %d\n", s.Cmd, s.CpuTimeSec)
		}
	}
}

func consumeOldSysinfo(info *oldfmt.SysinfoEnvelope) {
	fmt.Printf("Sysinfo for %s %s\n", info.Hostname, info.Timestamp)
	fmt.Printf("  Desc %s\n  CpuCores %d\n  MemGB %d\n", info.Description, info.CpuCores, info.MemGB)
}
