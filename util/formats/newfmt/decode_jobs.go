package newfmt

import (
	"encoding/json"
	"errors"
	"io"
	"strconv"
	"strings"
)

// The data are not comma-separated or in an array.  So we must decode one at a time.
//
// NOTE that if we add job types then we may need to add logic here, depending a little on
// representations.

func ConsumeJSONJobs(input io.Reader, strict bool, consume func(*JobsEnvelope)) error {
	dec := json.NewDecoder(input)
	if strict {
		dec.DisallowUnknownFields()
	}
	for dec.More() {
		info := new(JobsEnvelope)
		err := dec.Decode(info)
		if err != nil {
			return err
		}
		if info.Data != nil && len(info.Errors) > 0 {
			return errors.New("Can't have both Data and Errors")
		}
		consume(info)
	}
	return nil
}

// Given a standard encoding of Slurm TRES data, return it as a key/value list.
//
// The field format is defined by the slurm.conf man page as a comma-separated list of key=value
// pairs, with the implication that commas do not appear in the field (and that if there are quotes,
// they are part of the value).  But note that it is an ordered list.  Here's an example:
//
//   billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1

// The "Value" is an int64 if it can be parsed as that, otherwise float64 if it can be parsed as
// that, otherwise string.  That includes values suffixed by "G", "M", or "K": "50G" above is parsed
// as an i64 with the value 50*2^30; "45.50M" would be 45.5*2^20.  Should the value overflow, the
// parser falls back to a string.

type SlurmTRES struct {
	Key   string
	Value any
}

func DecodeSlurmTRES(s string) (result []SlurmTRES, dropped []string) {
	for _, pair := range strings.Split(s, ",") {
		k, kv, found := strings.Cut(pair, "=")
		if !found {
			dropped = append(dropped, pair)
			continue
		}
		var value any
		var scale int64 = 1
		v := kv
		if before, found := strings.CutSuffix(v, "G"); found {
			v = before
			scale = 1024 * 1024 * 1024
		} else if before, found := strings.CutSuffix(v, "M"); found {
			v = before
			scale = 1024 * 1024
		} else if before, found := strings.CutSuffix(v, "K"); found {
			v = before
			scale = 1024
		}
		if i, err := strconv.ParseInt(v, 10, 64); err == nil {
			value = i * scale
		} else if f, err := strconv.ParseFloat(v, 64); err == nil {
			value = f * float64(scale)
		} else {
			value = kv
		}
		result = append(result, SlurmTRES{k, value})
	}
	return
}
