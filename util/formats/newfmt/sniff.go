// SPDX-License-Identifier: MIT

// Copyright (c) 2023-2026 Norwegian Ai Cloud

package newfmt

import (
	"encoding/json"
	"errors"
	"io"
)

// SniffType tries to read a JSON object from the input and returns its type tag if it can be read.
// It will return io.EOF if there is no JSON following the current position, or another type of
// error if the data at the input position could not be decoded.  It will attempt to rewind the file
// to the current position before returning but will not catch an error in rewinding.
func SniffType(f io.ReadSeeker) (DataType, error) {
	type ToplevelData struct {
		Type DataType `json:"type"`
	}

	type Envelope struct {
		Meta MetadataObject `json:"meta"`
		Data *ToplevelData  `json:"data"`
	}

	here, err := f.Seek(0, io.SeekCurrent)
	if err != nil {
		return "", err
	}
	defer f.Seek(here, io.SeekStart)
	dec := json.NewDecoder(f)
	var m Envelope
	err = dec.Decode(&m)
	if err != nil {
		return "", err
	}
	if m.Data == nil || m.Data.Type == "" {
		return "", errors.New("Invalid metadata object")
	}
	return m.Data.Type, nil
}
