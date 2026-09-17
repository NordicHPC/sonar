package main

import (
	"encoding/json"
	"errors"

	"github.com/NordicHPC/sonar/util/formats/newfmt"
	"os"
)

// Given a file at position 0 that could contain Sonar data, try to find out if it does.  Returns
// the type tag if so.  Will return io.EOF if the file is empty.  Will attempt to rewind the file
// before returning but will not catch an error in rewinding.
func SniffType(f *os.File) (newfmt.DataType, error) {
	type ToplevelData struct {
		Type newfmt.DataType `json:"type"`
	}

	type Envelope struct {
		Meta newfmt.MetadataObject `json:"meta"`
		Data *ToplevelData         `json:"data"`
	}

	defer f.Seek(0, 0)
	dec := json.NewDecoder(f)
	var m Envelope
	err := dec.Decode(&m)
	if err != nil {
		return "", err
	}
	if m.Data == nil || m.Data.Type == "" {
		return "", errors.New("Invalid metadata object")
	}
	return m.Data.Type, nil
}
