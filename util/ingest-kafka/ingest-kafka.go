// `ingest-kafka` will listen for new-format Sonar traffic from a kafka broker, and "do something"
// with the data (currently store it in a directory tree).
//
// See comments in ../../doc/HOWTO-KAFKA for an example of how to use this.
//
// In the present directory there are files sonar-nonslurm-node.cfg, sonar-slurm-node.cfg and
// sonar-slurm-master.cfg that set up the Sonar daemon on compute nodes and a cluster master
// respectively.  See comments in those files for how to adapt them to your use.

package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"os"
	"path"
	"time"

	"github.com/NordicHPC/sonar/util/formats/newfmt"
	"github.com/twmb/franz-go/pkg/kgo"
)

var (
	cluster = flag.String("cluster", "", "Cluster whose data we listen for")
	dataDir = flag.String("data-dir", "", "Directory under which to store data keyed by date and host")
	broker  = flag.String("broker", "localhost:9092", "Broker `host:port`")
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

	topicNames := make([]string, 0)
	for n := range topics {
		topicNames = append(topicNames, n)
	}
	cl, err := kgo.NewClient(
		kgo.SeedBrokers(*broker),
		kgo.ConsumerGroup("sonar-ingest"),
		kgo.ConsumeTopics(topicNames...),
	)
	if err != nil {
		log.Fatalf("%s: Failed to create client", *cluster, err)
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
				log.Printf("%s: SOFT ERROR: Topic handler failed", *cluster, record.Topic, err)
			}
		}
		if err := cl.CommitUncommittedOffsets(ctx); err != nil {
			log.Printf("%s: SOFT ERROR: Commit records failed", *cluster, err)
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
		return appendToFile(newfmt.DataTagSample, info.Data.Attributes.Node, info.Data.Attributes.Time, data)
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
		return appendToFile(newfmt.DataTagSysinfo, info.Data.Attributes.Node, info.Data.Attributes.Time, data)
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
		return appendToFile(newfmt.DataTagJobs, info.Data.Attributes.Cluster, info.Data.Attributes.Time, data)
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
		return appendToFile(newfmt.DataTagCluster, info.Data.Attributes.Cluster, info.Data.Attributes.Time, data)
	}
	reportError(newfmt.DataTagCluster, info.Errors)
	return nil
}

func appendToFile(tag newfmt.DataType, host newfmt.Hostname, timestamp newfmt.Timestamp, data []byte) error {
	basename := string(tag) + "-" + string(host) + ".json"
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
