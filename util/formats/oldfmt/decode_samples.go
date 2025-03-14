// Decode old sample (aka `sonar ps`) data.
//
// Old CSV data are delivered in batches where the first record can carry additional information
// about the batch as a whole (`load`, `gpuinfo`).
//
// Data from v0.6.0 and earlier are quietly ignored (fixed field positions, no field names) but this
// is easy to fix.
//
// Adjacent records from the same time and host are consolidated into a single envelope here.  We do
// not purge copies of records, should they appear in the data.
//
// This just uses the Go CSV parser b/c perf is not considered much of an issue for the use cases
// this will be used for, but we could instead use the optimized CSV parser in sonalyze.

package oldfmt

import (
	"encoding/csv"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strconv"
	"strings"
)

func ConsumeCSVSamples(
	input io.Reader,
	consume func(*SampleEnvelope),
) error {
	r := csv.NewReader(input)
	r.FieldsPerRecord = -1 // variable
	var lineno int
	var envelope *SampleEnvelope
	currentTime := ""
	currentHost := ""
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
			continue
		}
		// Very-old-format records (v0.6 and earlier) start with the digit `2`, the initial digit in
		// the timestamp.  Ignore these records.
		if strings.HasPrefix(fields[0], "2") {
			continue
		}

		m := make(map[string]string)
		for _, fld := range fields {
			key, value, found := strings.Cut(fld, "=")
			if !found {
				continue
			}
			m[key] = value
		}

		if _, found := m["v"]; !found {
			continue
		}
		t, found := m["time"]
		if !found {
			continue
		}
		h, found := m["host"]
		if !found {
			continue
		}
		if t != currentTime || h != currentHost {
			if envelope != nil {
				consume(envelope)
			}
			envelope = new(SampleEnvelope)
			currentTime = t
			currentHost = h
		}

		var sample ProcessSample
		for k, v := range m {
			err = nil
			switch k {
			case "cores":
				envelope.Cores, err = strconv.ParseUint(v, 10, 64)
			case "cmd":
				sample.Cmd = v
			case "cpu%":
				sample.CpuPct, err = strconv.ParseFloat(v, 64)
			case "cpukib":
				sample.CpuKib, err = strconv.ParseUint(v, 10, 64)
			case "cputime_sec":
				sample.CpuTimeSec, err = strconv.ParseUint(v, 10, 64)
			case "gpu%":
				sample.GpuPct, err = strconv.ParseFloat(v, 64)
			case "gpufail":
				sample.GpuFail, err = strconv.ParseUint(v, 10, 64)
			case "gpuinfo":
				envelope.GpuSamples, err = DecodeGpuSamples([]byte(v))
			case "gpukib":
				sample.GpuKib, err = strconv.ParseUint(v, 10, 64)
			case "gpumem%":
				sample.GpuMemPct, err = strconv.ParseFloat(v, 64)
			case "gpus":
				sample.Gpus = v
			case "host":
				envelope.Hostname = v
			case "job":
				sample.JobId, err = strconv.ParseUint(v, 10, 64)
			case "load":
				envelope.CpuLoad, err = DecodeLoadData([]byte(v))
			case "memtotalkib":
				envelope.MemtotalKib, err = strconv.ParseUint(v, 10, 64)
			case "pid":
				sample.Pid, err = strconv.ParseUint(v, 10, 64)
			case "ppid":
				sample.ParentPid, err = strconv.ParseUint(v, 10, 64)
			case "rssanonkib":
				sample.RssAnonKib, err = strconv.ParseUint(v, 10, 64)
			case "rolledup":
				sample.Rolledup, err = strconv.ParseUint(v, 10, 64)
			case "time":
				envelope.Timestamp = v
			case "user":
				sample.User = v
			case "v":
				envelope.Version = v
			}
			if err != nil {
				continue Reader
			}
		}
		envelope.Samples = append(envelope.Samples, sample)
	}
	if envelope != nil {
		consume(envelope)
	}
	return nil
}

// Decode nested-encoded GPU sample information, see Sonar documentation.
//
// The input is a comma-separated string of arrays, each array represented as a substring
// tag=x|y|...|z, where the array fields contain no ",".  The tag identifies the field:
//
// fan%=27|28|28,perf=P8|P8|P8,musekib=1024|1024|1024,tempc=26|27|28,poww=5|2|20,powlimw=250|250|250,cez=300|300|300,memz=405|405|405
//
// Note zero can be denoted by the empty string, which can be confusing.
//
// This was copied from sonalyze/sonarlog/postprocess.go in the Jobanalyzer implementation and
// subsequently bugfixed.

func DecodeGpuSamples(data []byte) (result []GpuSample, err error) {
	for _, f := range strings.Split(string(data), ",") {
		tag, adata, _ := strings.Cut(f, "=")
		data := strings.Split(adata, "|")
		if result == nil {
			result = make([]GpuSample, len(data))
		}
		for i := 0; i < len(data); i++ {
			err = nil
			switch tag {
			case "fan%":
				result[i].FanPct, err = strconv.ParseUint(data[i], 10, 64)
			case "perf":
				result[i].PerfState = data[i]
			case "mode":
				result[i].ComputeMode = data[i]
			case "musekib":
				result[i].MemUse, err = strconv.ParseUint(data[i], 10, 64)
			case "cutil%":
				result[i].CEUtilPct, err = strconv.ParseUint(data[i], 10, 64)
			case "mutil%":
				result[i].MemUtilPct, err = strconv.ParseUint(data[i], 10, 64)
			case "tempc":
				result[i].Temp, err = strconv.ParseUint(data[i], 10, 64)
			case "poww":
				result[i].Power, err = strconv.ParseUint(data[i], 10, 64)
			case "powlimw":
				result[i].PowerLimit, err = strconv.ParseUint(data[i], 10, 64)
			case "cez":
				result[i].CEClock, err = strconv.ParseUint(data[i], 10, 64)
			case "memz":
				result[i].MemClock, err = strconv.ParseUint(data[i], 10, 64)
			}
			if err != nil && data[i] != "" {
				return
			}
		}
	}
	return
}

// Decode the list of per-GPU load data (cpu time used per core since boot).  This is base-45
// delta-encoded data, see Sonar documentation.
//
// This code was copied from sonalyze/sonarlog/postprocess.go in the Jobanalyzer implementation.

const (
	base       = 45
	none       = uint8(255)
	initial    = "(){}[]<>+-abcdefghijklmnopqrstuvwxyz!@#$%^&*_"
	subsequent = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ~|';:.?/`"
)

var (
	initialVal    = make([]byte, 256)
	subsequentVal = make([]byte, 256)
	decodeError   = errors.New("Could not decode load datum")
	noDataError   = errors.New("Empty data array")
)

func init() {
	for i := 0; i < 255; i++ {
		initialVal[i] = none
		subsequentVal[i] = none
	}
	for i := byte(0); i < base; i++ {
		initialVal[initial[i]] = i
		subsequentVal[subsequent[i]] = i
	}
}

func DecodeLoadData(data []byte) ([]uint64, error) {
	var (
		// shift==0 means no value
		val, shift uint64
		vals       = make([]uint64, 0, len(data)*3)
	)
	for _, c := range data {
		if initialVal[c] != none {
			if shift != 0 {
				vals = append(vals, val)
			}
			val = uint64(initialVal[c])
			shift = base
			continue
		}
		if subsequentVal[c] == none {
			return nil, decodeError
		}
		val += uint64(subsequentVal[c]) * shift
		shift *= base
	}
	if shift != 0 {
		vals = append(vals, val)
	}
	if len(vals) == 0 {
		return nil, noDataError
	}
	minVal := vals[0]
	for i := 1; i < len(vals); i++ {
		vals[i] += minVal
	}
	return vals[1:], nil
}

// Decode the old "gpus list", a list of the GPUs used by a process.  The value is nil iff
// the set contents are unknown.

type GpusList = []int

func DecodeGpusList(gpus string) (GpusList, error) {
	switch gpus {
	case "unknown":
		return nil, nil
	case "none", "":
		return make([]int, 0), nil
	default:
		xs := strings.Split(gpus, ",")
		ys := make([]int, len(xs))
		var err error
		for i, x := range xs {
			ys[i], err = strconv.Atoi(x)
			if err != nil {
				return nil, fmt.Errorf("Garbage in gpu list: %s", x)
			}
		}
		return ys, nil
	}
}

// The data are not comma-separated or in an array.  So we must decode one at a time.

func ConsumeJSONSamples(input io.Reader, consume func(*SampleEnvelope)) error {
	dec := json.NewDecoder(input)
	for dec.More() {
		info := new(SampleEnvelope)
		err := dec.Decode(info)
		if err != nil {
			return err
		}
		consume(info)
	}
	return nil
}
