// SPDX-License-Identifier: MIT

package oldfmt

import (
	"encoding/json"
	"io"
)

// The data are not comma-separated or in an array.  So we must decode one at a time.

func ConsumeJSONSysinfo(input io.Reader, consume func(*SysinfoEnvelope)) error {
	dec := json.NewDecoder(input)
	for dec.More() {
		info := new(SysinfoEnvelope)
		err := dec.Decode(info)
		if err != nil {
			return err
		}
		consume(info)
	}
	return nil
}
