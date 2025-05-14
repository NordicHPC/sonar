// The use case for this translation code is transitional - when we have old data files (or more
// generally are running an older Sonar) and want to store the old data in a new database or send
// the old data to the Kafka broker, which only wants new data.
//
// Old -> New is harder than New -> Old:
//
// - For "sysinfo", The old data don't have OsName, OsRelease, Architecture, TopoSVG, Software,
//   Cluster.  Of those, only OsName, OsRelease and Cluster matter much and can be passed as optional
//   parameters to the translator.  The old data also don't quite have Sockets, CoresPerSocket,
//   ThreadsPerCore or CpuModel, but we can parse those from the Description, which has been the same
//   since time immemorial.
//
// - For "cluster", old data simply do not contain these data, and they would need to be partly
//   reconstructed from other data if we really want them: node names from sysinfo data and partition
//   names from slurm jobs data.

package newfmt

import (
	"regexp"
	"strconv"
	"strings"

	"github.com/NordicHPC/sonar/util/formats/oldfmt"
)

func toNonemptyString(s string) NonemptyString {
	if s == "" {
		panic("Empty string")
	}
	return NonemptyString(s)
}

func toTimestamp(s string) Timestamp {
	// TODO - format check?
	return Timestamp(s)
}

func toHostname(s string) Hostname {
	// TODO - format check?
	return Hostname(s)
}

func toNonzeroUint(u uint64) NonzeroUint {
	if u == 0 {
		panic("Zero")
	}
	return NonzeroUint(u)
}

// TODO: Samples
// TODO: Jobs

type OldSysinfoAdapter struct {
	Cluster      string // Must be provided
	OsName       string // Default "Linux"
	OsRelease    string // Default "4.18.0"
	Architecture string // Default "x86_64"
}

var descMatcher = regexp.MustCompile(`^(\d+)x(\d+)( \(hyperthreaded\))?(.*?), \d+ GiB`)

func OldSysinfoToNew(d *oldfmt.SysinfoEnvelope, adapter OldSysinfoAdapter) (n SysinfoEnvelope) {
	if adapter.Cluster == "" {
		panic("The adapter must have cluster")
	}
	if adapter.Architecture == "" {
		adapter.Architecture = "x86_64"
	}
	if adapter.OsName == "" {
		adapter.OsName = "Linux"
	}
	if adapter.OsRelease == "" {
		adapter.OsRelease = "4.18.0"
	}
	n.Meta.Producer = "sonar"
	n.Meta.Version = toNonemptyString(d.Version)
	if d.CpuCores == 0 && d.MemGB == 0 {
		n.Errors = []ErrorObject{
			ErrorObject{
				Time:    toTimestamp(d.Timestamp),
				Detail:  toNonemptyString(d.Description),
				Cluster: toHostname(adapter.Cluster),
				Node:    toHostname(d.Hostname),
			},
		}
	} else {
		n.Data = new(SysinfoData)
		n.Data.Type = DataTagSysinfo
		a := &n.Data.Attributes
		a.Time = toTimestamp(d.Timestamp)
		a.Cluster = toHostname(adapter.Cluster)
		a.Node = toHostname(d.Hostname)
		a.OsName = toNonemptyString(adapter.OsName)
		a.OsRelease = toNonemptyString(adapter.OsRelease)
		// Maybe the description contains some hints, but we don't actually care.
		a.Architecture = toNonemptyString(adapter.Architecture)
		// Not clear if we can continue if the match fails.  But don't know why it would fail.
		if m := descMatcher.FindStringSubmatch(d.Description); m != nil {
			n, _ := strconv.ParseUint(m[1], 10, 64)
			a.Sockets = toNonzeroUint(n)
			n, _ = strconv.ParseUint(m[2], 10, 64)
			a.CoresPerSocket = toNonzeroUint(n)
			var threads uint64 = 1
			if m[3] != "" {
				threads = 2
			}
			a.ThreadsPerCore = toNonzeroUint(threads)
			a.CpuModel = strings.TrimSpace(m[4])
		}
		a.Memory = toNonzeroUint(d.MemGB * 1024 * 1024)
		// TopoSVG is not possible
		if len(d.GpuInfo) > 0 {
			a.Cards = make([]SysinfoGpuCard, len(d.GpuInfo))
			for i, c := range d.GpuInfo {
				n := &a.Cards[i]
				n.Index = c.Index
				n.UUID = c.UUID
				n.Address = c.BusAddress
				n.Manufacturer = c.Manufacturer
				n.Model = c.Model
				n.Architecture = c.Architecture
				n.Driver = c.Driver
				n.Firmware = c.Firmware
				n.Memory = c.MemKB
				n.PowerLimit = c.PowerLimit
				n.MaxPowerLimit = c.MaxPowerLimit
				n.MinPowerLimit = c.MinPowerLimit
				n.MaxCEClock = c.MaxCEClock
				n.MaxMemoryClock = c.MaxMemClock
			}
		}
		// Software is not possible
	}
	return
}
