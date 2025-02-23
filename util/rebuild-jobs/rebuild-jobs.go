// Read a sequence of slurm jobs data packages - typically delta-coded - from a set of files and
// rebuild the full data stream.
//
// For a non-delta-coded set of data, this should in principle be a no-op, but data that are sent
// redundantly may actually be filtered.
//
// For a delta-coded set of data, the resulting stream should be minimal - there should be no
// redundant records for completed jobs, though for pending/running jobs there can be.
//
// Usage:
//   rebuild-jobs filename ...
//
// The files can be in any order but should all be for a single cluster.  The records will be sorted
// by their timestamps, and then for each (partial) record X in the resulting stream, fields from
// the previous full record will be taken as background to build the full record for X.  See below
// for a description of the algorithm.

package main

import (
	"cmp"
	"encoding/json"
	"fmt"
	"log"
	"os"
	"slices"

	"github.com/NordicHPC/sonar/util/formats/newfmt"
)

func main() {
	records := make([]*newfmt.JobsEnvelope, 0)
	for _, filename := range os.Args[1:] {
		f, err := os.Open(filename)
		if err != nil {
			log.Printf("Can't open %s", filename)
			continue
		}
		err = newfmt.ConsumeJSONJobs(f, false, func(e *newfmt.JobsEnvelope) {
			if e.Data != nil {
				records = append(records, e)
			}
		})
		f.Close()
		if err != nil {
			log.Printf("Error reading %s %s", filename, err)
			continue
		}
	}

	if len(records) == 0 {
		return
	}

	slices.SortFunc(records, func (a, b *newfmt.JobsEnvelope) int {
		return cmp.Compare(a.Data.Attributes.Time, b.Data.Attributes.Time)
	})

	// Redundant time stamps can happen if:
	//
	//  - data were split before transmission (not currently something we do)
	//  - data were sent redundantly eg if the daemon crashed and restarted within the same second
	//    (not likely but theoretically possible)
	//
	// Thus redundant timestamps are currently errors and we arbitrarily discard duplicates,
	// CompactFunc will keep the first in a run.

	records = slices.CompactFunc(records, func (a, b *newfmt.JobsEnvelope) bool {
		return a.Data.Attributes.Time == b.Data.Attributes.Time
	})

	// So for a given JobsAttributes object x, we want to reconstruct a full x.SlurmJobs based on
	// the partial x.SlurmJobs and the full data from the most recently reconstructed object y and
	// its y.SlurmJobs.
	//
	// A "record" is identified uniquely by its (jobid,jobstep) key.
	//
	// If a record A with key K is in x.SlurmJobs and a record B with key K is in y.SlurmJobs then
	// there are two cases:
	//
	// * if A.state and B.state are both completed, then remove A from x.SlurmJobs: it must be
	//   redundant data.  This would not usually be the case for delta-encoded data but it could
	//   happen if the daemon is restarted.  For non-delta-encoded data it might happen often.
	// * otherwise, for each of the fields of A that can be elided by compression, if the field
	//   has a default value, copy the value from B, and finally leave the modified A in x.SlurmJobs.
	//
	// If a record A with key K in y.SlurmJobs whose A.state is not completed and there is no record
	// with key K in x.SlurmJobs, then copy A into x.SlurmJobs.
	//
	// If a record A with key K is in x.SlurmJobs but there is no record with key K in y.SlurmJobs
	// then we leave A unchanged in x.SlurmJobs.

	y := &records[0].Data.Attributes
	for _, xr := range records[1:] {
		x := &xr.Data.Attributes
		for ix, a := range x.SlurmJobs {
			if b := find(y, a.JobID, a.JobStep); b != nil {
				if a.JobState == b.JobState && isCompleted(a.JobState) {
					x.SlurmJobs[ix].JobID = 0
				} else {
					// The fields that can be elided are defined in ../../src/slurmjobs.rs in the
					// function filter_jobs.
					a.JobName = defString(a.JobName, b.JobName)
					a.JobState = defString(a.JobState, b.JobState)
					a.UserName = defString(a.UserName, b.UserName)
					a.Account = defString(a.Account, b.Account)
					a.SubmitTime = defString(a.SubmitTime, b.SubmitTime)
					a.Timelimit = defUint(a.Timelimit, b.Timelimit)
					a.Partition = defString(a.Partition, b.Partition)
					a.Reservation = defString(a.Reservation, b.Reservation)
					a.NodeList = defStrings(a.NodeList, b.NodeList)
					a.Priority = defUint(a.Priority, b.Priority)
					a.Layout = defString(a.Layout, b.Layout)
					a.GRESDetail = defStrings(a.GRESDetail, b.GRESDetail)
					a.ReqCPUS = defUint(a.ReqCPUS, b.ReqCPUS)
					a.MinCPUSPerNode = defUint(a.MinCPUSPerNode, b.MinCPUSPerNode)
					a.ReqMemoryPerNode = defUint(a.ReqMemoryPerNode, b.ReqMemoryPerNode)
					a.ReqNodes = defUint(a.ReqNodes, b.ReqNodes)
					a.Start = defString(a.Start, b.Start)
					a.Suspended = defUint(a.Suspended, b.Suspended)
					a.End = defString(a.End, b.End)
					a.ExitCode = defUint(a.ExitCode, b.ExitCode)
					if b.Sacct != nil {
						if a.Sacct == nil {
							a.Sacct = new(newfmt.SacctData)
						}
						sa := a.Sacct
						sb := b.Sacct
						sa.MinCPU = defUint(sa.MinCPU, sb.MinCPU)
						sa.AllocTRES = defString(sa.AllocTRES, sb.AllocTRES)
						sa.AveCPU = defUint(sa.AveCPU, sb.AveCPU)
						sa.AveDiskRead = defUint(sa.AveDiskRead, sb.AveDiskRead)
						sa.AveDiskWrite = defUint(sa.AveDiskWrite, sb.AveDiskWrite)
						sa.AveRSS = defUint(sa.AveRSS, sb.AveRSS)
						sa.AveVMSize = defUint(sa.AveVMSize, sb.AveVMSize)
						sa.ElapsedRaw = defUint(sa.ElapsedRaw, sb.ElapsedRaw)
						sa.SystemCPU = defUint(sa.SystemCPU, sb.SystemCPU)
						sa.UserCPU = defUint(sa.UserCPU, sb.UserCPU)
						sa.MaxRSS = defUint(sa.MaxRSS, sb.MaxRSS)
						sa.MaxVMSize = defUint(sa.MaxVMSize, sb.MaxVMSize)
					}
				}
			}
		}

		x.SlurmJobs = deleteFuncRef(x.SlurmJobs, func(v *newfmt.SlurmJob) bool {
			return v.JobID == 0
		})

		for _, b := range y.SlurmJobs {
			if !isCompleted(b.JobState) {
				if a := find(x, b.JobID, b.JobStep); a == nil {
					x.SlurmJobs = append(x.SlurmJobs, b)
				}
			}
		}

		y = x
	}

	for _, r := range records {
		bs, err := json.Marshal(r)
		if err != nil {
			panic(err)
		}
		fmt.Println(string(bs))
	}
}

func defString[T ~string](a, b T) T {
	if a == "" {
		return b
	}
	return a
}

func defStrings(a, b []string) []string {
	if len(a) == 0 {
		return b
	}
	return a
}

func defUint[T ~uint64](a, b T) T {
	if a == 0 {
		return b
	}
	return a
}

func isCompleted(state newfmt.NonemptyString) bool {
	return state != "PENDING" && state != "RUNNING"
}

// This is called from within loops and will tend towards O(n^2) behavior, we'd be better off
// creating a hash table in a single pass and then looking into that.  Note that if we do that the
// append() in the second inner loop above may have to be done after the loop is done, which would
// be fine but requires an addtional slice to hold the elements to append.

func find(v *newfmt.JobsAttributes, id newfmt.NonzeroUint, step string) *newfmt.SlurmJob {
	for i := range v.SlurmJobs {
		if v.SlurmJobs[i].JobID == id && v.SlurmJobs[i].JobStep == step {
			return &v.SlurmJobs[i]
		}
	}
	return nil
}

func deleteFuncRef[E any](xs []E, f func(*E) bool) []E {
	dest := 0
	for src := range xs {
		if !f(&xs[src]) {
			xs[dest] = xs[src]
			dest++
		}
		src++
	}
	clear(xs[dest:])
	return xs[:dest]
}
