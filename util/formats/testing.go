// SPDX-License-Identifier: MIT

// Copyright (c) 2023-2026 Norwegian Ai Cloud

package formats

import (
	"testing"
)

func assert(t *testing.T, c bool, msg string) {
	if !c {
		t.Fatal(msg)
	}
}
