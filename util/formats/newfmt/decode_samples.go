// SPDX-License-Identifier: MIT

// Copyright (c) 2023-2026 Norwegian Ai Cloud

package newfmt

import (
	"encoding/json"
	"errors"
	"io"
)

// ConsumeJSONSamples reads Sonar Sample data from a file and calls the consumer on each object.
// The data in the file are not comma-separated or in an array, just a sequence of them.
func ConsumeJSONSamples(input io.Reader, strict bool, consume func(*SampleEnvelope)) error {
	dec := json.NewDecoder(input)
	if strict {
		dec.DisallowUnknownFields()
	}
	for dec.More() {
		info := new(SampleEnvelope)
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
