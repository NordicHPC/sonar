// Ingest-kafka will listen for new-format Sonar traffic from a Kafka broker, and "do something"
// with the data (currently this program stores it in a directory tree).
//
// Usage:
//
//	ingest-kafka [options]
//
// where required options are
//
//	-cluster cluster-name
//	  The cluster whose data we listen for
//
//	-data-dir directory
//	  The data directory under which to store data
//
// and optional options are
//
//	-broker broker-address
//	  The host:port on which the broker listens
//
// See comments in ../../doc/HOWTO-KAFKA for an example of how to use this.
//
// BUGS:
//
// Currently, ingest-kafka uses a file naming scheme where data for, say, the "sample" type are
// appended to the file yyyy/mm/dd/sample-<cluster-name>.json, where the cluster name and the
// yyyy-mm-dd time stamp come from each datum.  This is incompatible with how Sonalyze stores the
// data and on large clusters it will also lead to very large files where data for many individual
// hosts are in the same file.
package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"maps"
	"os"
	"path"
	"slices"
	"time"

	"github.com/NordicHPC/sonar/util/formats/newfmt"
	"github.com/twmb/franz-go/pkg/kgo"
)

var (
	cluster = flag.String("cluster", "", "Listen for data from `cluster-name`")
	dataDir = flag.String("data-dir", "", "Root of `data-directory` below which we store data keyed by date and host")
	broker  = flag.String("broker", "localhost:9092", "Kafka `broker-address` on \"host:port\" format")
	verbose = flag.Bool("v", false, "Verbose")
)

func main() {
	flag.Parse()
	if *cluster == "" {
		fmt.Fprintln(os.Stderr, "The -cluster is required")
		os.Exit(2)
	}
	if *dataDir == "" {
		fmt.Fprintln(os.Stderr, "The -data-dir is required")
		os.Exit(2)
	}

	var topics = map[string]func([]byte) error{
		*cluster + "." + string(newfmt.DataTagSample):  handleSample,
		*cluster + "." + string(newfmt.DataTagSysinfo): handleSysinfo,
		*cluster + "." + string(newfmt.DataTagJobs):    handleJobs,
		*cluster + "." + string(newfmt.DataTagCluster): handleCluster,
	}

	cl, err := kgo.NewClient(
		kgo.SeedBrokers(*broker),
		kgo.ConsumerGroup("sonar-ingest"),
		kgo.ConsumeTopics(slices.Collect(maps.Keys(topics))...),
	)
	if err != nil {
		log.Fatalf("%s: Failed to create client: %v", *cluster, err)
	}
	defer cl.Close()
	if *verbose {
		log.Printf("%s: Connected!", *cluster)
	}

	ctx := context.Background()
	for {
		if *verbose {
			log.Printf("%s: Fetching data", *cluster)
		}
		fetches := cl.PollFetches(ctx)
		if *verbose {
			log.Printf("%s: Fetched data", *cluster)
		}
		if errs := fetches.Errors(); len(errs) > 0 {
			// All errors are retried internally when fetching, but non-retriable errors are
			// returned from polls so that users can notice and take action.
			log.Printf("%s: SOFT ERROR: Failed to fetch data! %v", *cluster, errs)
		}

		iter := fetches.RecordIter()
		for !iter.Done() {
			record := iter.Next()
			if *verbose {
				log.Printf("  %s: %s", *cluster, record.Topic)
			}
			err := topics[record.Topic](record.Value)
			if err != nil {
				log.Printf("%s: SOFT ERROR: Topic handler for %s failed: %v", *cluster, record.Topic, err)
			}
		}
		if err := cl.CommitUncommittedOffsets(ctx); err != nil {
			log.Printf("%s: SOFT ERROR: Commit records failed: %v", *cluster, err)
		}
	}
}

func handleSample(data []byte) error {
	info := new(newfmt.SampleEnvelope)
	err := json.Unmarshal(data, info)
	if err != nil {
		return err
	}
	if info.Data != nil {
		return appendToFile(
			newfmt.DataTagSample,
			newfmt.NonemptyString(info.Data.Attributes.Node),
			info.Data.Attributes.Time,
			data,
		)
	}
	reportError(newfmt.DataTagSample, info.Errors)
	return nil
}

func handleSysinfo(data []byte) error {
	info := new(newfmt.SysinfoEnvelope)
	err := json.Unmarshal(data, info)
	if err != nil {
		return err
	}
	if info.Data != nil {
		return appendToFile(
			newfmt.DataTagSysinfo,
			newfmt.NonemptyString(info.Data.Attributes.Node),
			info.Data.Attributes.Time,
			data,
		)
	}
	reportError(newfmt.DataTagSysinfo, info.Errors)
	return nil
}

func handleJobs(data []byte) error {
	info := new(newfmt.JobsEnvelope)
	err := json.Unmarshal(data, info)
	if err != nil {
		return err
	}
	if info.Data != nil {
		return appendToFile(
			newfmt.DataTagJobs,
			newfmt.NonemptyString(info.Data.Attributes.Cluster),
			info.Data.Attributes.Time,
			data,
		)
	}
	reportError(newfmt.DataTagJobs, info.Errors)
	return nil
}

func handleCluster(data []byte) error {
	info := new(newfmt.ClusterEnvelope)
	err := json.Unmarshal(data, info)
	if err != nil {
		return err
	}
	if info.Data != nil {
		return appendToFile(
			newfmt.DataTagCluster,
			newfmt.NonemptyString(info.Data.Attributes.Cluster),
			info.Data.Attributes.Time,
			data,
		)
	}
	reportError(newfmt.DataTagCluster, info.Errors)
	return nil
}

func appendToFile(
	tag newfmt.DataType,
	hostOrCluster newfmt.NonemptyString,
	timestamp newfmt.Timestamp,
	data []byte,
) error {
	basename := string(tag) + "-" + string(hostOrCluster) + ".json"
	t, err := time.Parse(time.RFC3339, string(timestamp))
	if err != nil {
		return err
	}
	timedir := t.Format("2006/01/02")
	err = os.MkdirAll(path.Join(*dataDir, timedir), 0o777)
	if err != nil {
		return err
	}
	filename := path.Join(*dataDir, timedir, basename)
	f, err := os.OpenFile(filename, os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o666)
	if err != nil {
		return err
	}
	defer f.Close()
	_, err = f.Write(data)
	if err != nil {
		return err
	}
	_, err = f.WriteString("\n")
	return err
}

func reportError(tag newfmt.DataType, errors []newfmt.ErrorObject) {
	for _, e := range errors {
		log.Printf("%s: %s: %s: %s / %s: Error: %s", *cluster, tag, e.Time, e.Cluster, e.Node, e.Detail)
	}
}
