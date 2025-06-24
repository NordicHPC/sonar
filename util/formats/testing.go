// SPDX-License-Identifier: MIT

package formats

import (
	"testing"
)

func assert(t *testing.T, c bool, msg string) {
	if !c {
		t.Fatal(msg)
	}
}
