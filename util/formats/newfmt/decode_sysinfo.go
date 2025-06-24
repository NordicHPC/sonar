// SPDX-License-Identifier: MIT

package newfmt

import (
	"encoding/json"
	"errors"
	"io"
)

// The data are not comma-separated or in an array.  So we must decode one at a time.

func ConsumeJSONSysinfo(input io.Reader, strict bool, consume func(*SysinfoEnvelope)) error {
	dec := json.NewDecoder(input)
	if strict {
		dec.DisallowUnknownFields()
	}
	for dec.More() {
		info := new(SysinfoEnvelope)
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
