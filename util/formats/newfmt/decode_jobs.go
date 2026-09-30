// SPDX-License-Identifier: MIT

// Copyright (c) 2023-2026 Norwegian Ai Cloud

package newfmt

import (
	"encoding/json"
	"errors"
	"io"
	"strconv"
	"strings"
)

// ConsumeJSONJobs reads Sonar Jobs data from a file and calls the consumer on each object.  The
// data in the file are not comma-separated or in an array, just a sequence of them.
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

// SlurmTRES encodes a Slurm TRES key/value pair.
type SlurmTRES struct {
	Key string
	// The Value is int64, float64, or string.
	Value any
}

// DecodeSlurmTRES takes a standard encoding of Slurm TRES data and returns it as a key/value list.
//
// The field format is defined by the slurm.conf man page as a comma-separated list of key=value
// pairs, with the implication that commas do not appear in the field (and that if there are quotes,
// they are part of the value).  But note that it is an ordered list.  Here's an example:
//
//	billing=20,cpu=20,gres/gpu:rtx30=1,gres/gpu=1,mem=50G,node=1
//
// The value is represented as int64 if it could be parsed as that, otherwise float64 if it could be
// parsed as that, otherwise string.  That includes values suffixed by "P", "T", "G", "M", or "K":
// "50G" above is parsed as an i64 with the value 50*2^30; "45.50M" would be 45.5*2^20.
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
		if before, found := strings.CutSuffix(v, "P"); found {
			v = before
			scale = 1024 * 1024 * 1024 * 1024 * 1024
		} else if before, found := strings.CutSuffix(v, "T"); found {
			v = before
			scale = 1024 * 1024 * 1024 * 1024
		} else if before, found := strings.CutSuffix(v, "G"); found {
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
