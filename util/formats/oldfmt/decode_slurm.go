package oldfmt

import (
	"encoding/csv"
	"encoding/json"
	"fmt"
	"io"
	"strings"
)

// The data are not comma-separated or in an array.  So we must decode one at a time.  The object
// passed to consume is either a SlurmEnvelope or a SlurmErrorEnvelope, never anything else.

func ConsumeJSONSlurmJobs(input io.Reader, consume func(any)) error {
	type slurmEnvelope struct {
		Version   string     `json:"version"`
		Jobs      []SlurmJob `json:"jobs"`
		Timestamp string     `json:"timestamp"`
		Error     string     `json:"error"`
	}

	dec := json.NewDecoder(input)
	var e slurmEnvelope
	for dec.More() {
		e.Error = ""
		err := dec.Decode(&e)
		if err != nil {
			return err
		}
		if e.Error != "" {
			consume(&SlurmErrorEnvelope{
				Version:   e.Version,
				Timestamp: e.Timestamp,
				Error:     e.Error,
			})
		} else {
			consume(&SlurmEnvelope{
				Version: e.Version,
				Jobs:    e.Jobs,
			})
		}
	}
	return nil
}

// There's some regrettable information loss in the input here: non-error records don't have
// timestamps, so we don't know which records "belong" together.  It's probably not important.  But
// we have two options: we can create one envelope per record, or we can collect records together
// that might belong together, for example, we collect everything until we see a JobID value that we
// already have in the package, or we see an error record.  Here I do the latter.

func ConsumeCSVSlurmJobs(input io.Reader, consume func(any)) error {
	r := csv.NewReader(input)
	r.FieldsPerRecord = -1 // variable
	var lineno int
	var envelope *SlurmEnvelope
	var known map[string]bool
Reader:
	for {
		lineno++
		fields, err := r.Read()
		if err == io.EOF {
			break
		}
		if err != nil {
			return fmt.Errorf("Error on line %d: %v", lineno, err)
		}
		if len(fields) == 0 {
			continue Reader
		}
		m := make(map[string]string)
	Fields:
		for _, fld := range fields {
			key, value, found := strings.Cut(fld, "=")
			if !found {
				continue Fields
			}
			m[key] = value
		}

		// Error flushes and moves to next
		if _, found := m["error"]; found {
			if envelope != nil {
				consume(envelope)
				envelope = nil
			}
			consume(&SlurmErrorEnvelope{
				Version:   m["v"],
				Timestamp: m["timestamp"],
				Error:     m["error"],
			})
			continue Reader
		}

		// Known JobID flushes but then continues down
		if envelope != nil && known[m["JobID"]] {
			consume(envelope)
			envelope = nil
		}

		if envelope == nil {
			envelope = new(SlurmEnvelope)
			envelope.Version = m["v"]
			known = make(map[string]bool)
		}

		// Build a new job and stuff it in the envelope
		var job SlurmJob
		known[m["JobID"]] = true
		job.JobID = m["JobID"]
		job.JobIDRaw = m["JobIDRaw"]
		job.User = m["User"]
		job.Account = m["Account"]
		job.State = m["State"]
		job.Start = m["Start"]
		job.End = m["End"]
		job.AveCPU = m["AveCPU"]
		job.AveDiskRead = m["AveDiskRead"]
		job.AveDiskWrite = m["AveDiskWrite"]
		job.AveRSS = m["AveRSS"]
		job.AveVMSize = m["AveVMSize"]
		job.ElapsedRaw = m["ElapsedRaw"]
		job.ExitCode = m["ExitCode"]
		job.Layout = m["Layout"]
		job.MaxRSS = m["MaxRSS"]
		job.MaxVMSize = m["MaxVMSize"]
		job.MinCPU = m["MinCPU"]
		job.ReqCPUS = m["ReqCPUS"]
		job.ReqMem = m["ReqMem"]
		job.ReqNodes = m["ReqNodes"]
		job.Reservation = m["Reservation"]
		job.Submit = m["Submit"]
		job.Suspended = m["Suspended"]
		job.SystemCPU = m["SystemCPU"]
		job.TimelimitRaw = m["TimelimitRaw"]
		job.UserCPU = m["UserCPU"]
		job.NodeList = m["NodeList"]
		job.Partition = m["Partition"]
		job.AllocTRES = m["AllocTRES"]
		job.Priority = m["Priority"]
		job.JobName = m["JobName"]
		envelope.Jobs = append(envelope.Jobs, job)
	}
	if envelope != nil {
		consume(envelope)
	}
	return nil
}
