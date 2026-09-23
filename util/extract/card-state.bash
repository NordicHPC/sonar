#!/usr/bin/env bash
#
# Extract the card-state streams from sonar sample data and print as flat JSON.
#
# Reads from file arguments in the order given, prints everything to stdout.  The file arguments
# must have only new-format sample records.
#
# The script silently discards error objects, ie, with no "data" component.
#
# To adapt the script to a use case, edit it (easier than generalizing it with arguments).
#
# An interesting problem here is that fields in the input are absent if they have zero or empty
# values: notably .ce_util and .memory_util but also .failing and .compute_mode.  We want to print
# zero/"" rather than null.  It would be nice to do this systematically, but here it's done ad-hoc.
#
# Also there are some customizations here: we select only a specific GPU and not all fields are
# printed.  See card-config.bash for a little more about this.

for fn in "$@"; do
    jq -r -c '
select(has("data")) |
.data.attributes |
.time as $time |
.node as $node |
(.system.gpus.[] |
 select(.uuid == "GPU-e51c955b-9544-8eb4-a016-0c46a9f2899d") |
 {"time":$time, "node":$node, "uuid":.uuid,
  "failing":(if .failing == null then 0 else .failing end),
  "compute_mode":(if .compute_mode == null then "" else .compute_mode end),
  "performance_state":.performance_state,
  "temperature":.temperature, "power":.power,
  "ce_util":(if .ce_util == null then 0 else .ce_util end),
  "ce_clock":.ce_clock,
  "memory":.memory,
  "memory_util":(if .memory_util == null then 0 else .memory_util end),
  "memory_clock":.memory_clock})
' < $fn
done

