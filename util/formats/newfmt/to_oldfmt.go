// The use case for this is transitional - when we receive new sysinfo but want to store it in old
// data files.  Note there is no old format for cluster data.

package newfmt

import (
	"fmt"
	"math"

	"github.com/NordicHPC/sonar/util/formats/oldfmt"
)

// TODO: Samples
// TODO: Jobs

func NewSysinfoToOld(d *SysinfoEnvelope) (o oldfmt.SysinfoEnvelope) {
	o.Version = string(d.Meta.Version)
	if d.Errors != nil {
		// The old format really does not have an error channel but we can simulate it.
		// We decree that if CpuCores == 0 && MemGB == 0 then it is an error object.
		e := d.Errors[0]
		o.Timestamp = string(e.Time)
		o.Hostname = string(e.Node)
		o.Description = "ERROR: " + string(e.Detail)
	} else {
		a := d.Data.Attributes
		o.Timestamp = string(a.Time)
		o.Hostname = string(a.Node)
		o.CpuCores = uint64(a.Sockets * a.CoresPerSocket * a.ThreadsPerCore)
		o.MemGB = uint64(math.Ceil(float64(a.Memory) / (1024 * 1024)))
		cards := a.Cards
		if cards != nil {
			o.GpuCards = uint64(len(cards))
			var kb uint64
			for _, c := range cards {
				kb += c.Memory
			}
			o.GpuMemGB = uint64(math.Ceil(float64(kb) / (1024 * 1024)))
			gpus := make([]oldfmt.GpuSysinfo, len(cards))
			for i, c := range cards {
				gpus[i].BusAddress = c.Address
				gpus[i].Index = c.Index
				gpus[i].UUID = c.UUID
				gpus[i].Manufacturer = c.Manufacturer
				gpus[i].Model = c.Model
				gpus[i].Architecture = c.Architecture
				gpus[i].Driver = c.Driver
				gpus[i].Firmware = c.Firmware
				gpus[i].MemKB = c.Memory
				gpus[i].PowerLimit = c.PowerLimit
				gpus[i].MaxPowerLimit = c.MaxPowerLimit
				gpus[i].MinPowerLimit = c.MinPowerLimit
				gpus[i].MaxCEClock = c.MaxCEClock
				gpus[i].MaxMemClock = c.MaxMemoryClock
			}
			o.GpuInfo = gpus
		}
		var ht string
		if a.ThreadsPerCore > 1 {
			ht = " (hyperthreaded)"
		}
		var gpuDesc string
		var i int
		for i < len(cards) {
			first := i
			for i < len(cards) &&
				cards[i].Model == cards[first].Model &&
				cards[i].Memory == cards[first].Memory {
				i++
			}
			memsize := "unknown"
			if cards[first].Memory > 0 {
				memsize = fmt.Sprint(uint64(math.Ceil(float64(cards[first].Memory) / (1024 * 1024))))
			}
			gpuDesc += fmt.Sprintf(", %dx %s @ %dGiB", i-first, cards[first].Model, memsize)
		}
		o.Description = fmt.Sprintf("%dx%d%s %s, %d GiB%s",
			a.Sockets, a.CoresPerSocket, ht, a.CpuModel, o.MemGB, gpuDesc)
	}
	return
}
