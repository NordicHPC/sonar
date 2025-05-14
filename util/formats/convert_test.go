// Test the data conversion layers (somewhat)

package formats

import (
	"os"
	"testing"

	"github.com/NordicHPC/sonar/util/formats/newfmt"
	"github.com/NordicHPC/sonar/util/formats/oldfmt"
)

func TestOldToNewSysinfo(t *testing.T) {
	f, err := os.Open("testdata/oldfmt_sysinfo.json")
	if err != nil {
		t.Fatal(err)
	}
	defer f.Close()
	var counter int
	oldfmt.ConsumeJSONSysinfo(f, func(info *oldfmt.SysinfoEnvelope) {
		// Basically, test that it works, but it would be helpful to check
		// some of the gnarlier fields
		adapter := newfmt.OldSysinfoAdapter{
			Cluster: "akebakken.no",
		}
		n := newfmt.OldSysinfoToNew(info, adapter)
		switch counter {
		case 0:
			a := n.Data.Attributes
			if a.OsName != "Linux" {
				t.Fatal("OsName")
			}
			if a.Cluster != "akebakken.no" {
				t.Fatal("Cluster")
			}
			if a.Sockets != 2 {
				t.Fatal("Sockets")
			}
			if a.CoresPerSocket != 14 {
				t.Fatal("Cores per socket")
			}
			if a.ThreadsPerCore != 2 {
				t.Fatal("Threads per core")
			}
			if len(a.Cards) != 3 {
				t.Fatal("Cards")
			}
			if a.Cards[1].UUID != "GPU-be013a01-364d-ca23-f871-206fe3f259ba" {
				t.Fatal("UUID")
			}
		case 1:
			a := n.Data.Attributes
			if a.Architecture != "x86_64" {
				t.Fatal("Architecture")
			}
			if a.Sockets != 2 {
				t.Fatal("Sockets")
			}
			if a.CoresPerSocket != 64 {
				t.Fatal("Cores per socket")
			}
			if a.ThreadsPerCore != 1 {
				t.Fatal("Threads per core")
			}
		}
		counter++
	})
	if counter != 2 {
		t.Fatalf("Expected 2 records but got %d", counter)
	}
}
