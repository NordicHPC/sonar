#!/usr/bin/env bash
#
# Extract the card-configuration streams from sonar sysinfo data and print as flat JSON.
#
# Reads from file arguments in the order given, prints everything to stdout.  The file arguments
# must have only new-format sysinfo records.
#
# The script silently discards error objects, ie, with no "data" component.
#
# To adapt the script to a use case, edit it (easier than generalizing it with arguments).
#
#
# Example.
#
# Extract card data from Fox GPU nodes for June 2026:
#
#   ./card-config.bash .../fox.educloud.no/2026/06/*/0+sysinfo-gpu*.json
#
#
# Customization.
#
# A couple of obvious customizations are these:
#
# * The primary key for a card-configuration stream is the UUID.  In the output we'll have the
#   different streams for all UUIDs in all the files intermingled.  It is easy to add a filter for a
#   card of interest, as has been indicated in the commented-out line in the jq script below.
#
# * In the same vein, it's possible to filter on any key (node, index, whatever) or multiple keys.
#
# * Different use cases want to see different fields, so when fields are added in the data
#   definition, we may also want to add them here.  But we don't normally want to print *all*
#   fields.  Here I've omitted "driver" and "firmware" (as an example).

for fn in "$@"; do
    jq -r -c '
select(has("data")) |
.data.attributes |
.time as $time |
.cluster as $cluster |
.node as $node |
(.cards.[] |
# select(.uuid == "GPU-e51c955b-9544-8eb4-a016-0c46a9f2899d") |
 {"time":$time, "cluster":$cluster, "node":$node,
  "index":.index, "uuid":.uuid, "address":.address, "manufacturer":.manufacturer,
  "model":.model, "architecture":.architecture, "memory":.memory, "power_limit":.power_limit,
  "max_power_limit":.max_power_limit, "min_power_limit":.min_power_limit,
  "max_ce_clock":.max_ce_clock,"max_memory_clock":.max_memory_clock})
' < $fn
done
