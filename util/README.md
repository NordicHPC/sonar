# Sonar data formats and utility code

## Data definitions

In `formats/` are definitions of the Sonar data formats, as high-level Go code, and simple parsers
for the most important of the formats.  Currently there is a definition for only a JSON format
(known as the "new" format since there were older formats than that).

## Utility programs

### Kafka REST proxy

In `kafka-proxy/` is a simple REST proxy for Kafka.  Nodes that are not able to exfiltrate data to a
Kafka broker directly because they are behind an http/https proxy can be configured to use a REST
protocol for exfiltration, and the kafka-proxy component can be configured to receive that traffic
and forward it to the Kafka broker.

### Kafka ingestor

In `ingest-kafka/` is a simple ingestor for Sonar data from Kafka, it will listen for traffic from
the broker and store the data in files in a directory tree using a simple naming scheme.

### Structured documentation processor

In `process-doc/` is a program that parses the data format files in `formats/` (in the form of Go
data definitions with structured comments) and extracts .md documentation, .yaml documentation (used
by slurm-monitor), and JSON field tags (used by Sonar itself).

### Data anonymizer

In `anonymize/` is a program that will systematically anonymize Sonar output (rewriting user names,
account names, and other things) so that output obtained on a live cluster can be published,
typically for use in testing and documentation.

### Misc stuff

In `config/` is some older, experimental name resolver code, it is used to debug node and cluster
host name configurations.  For the specially interested only.
