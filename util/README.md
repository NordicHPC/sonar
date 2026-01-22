In `formats/` are definitions of the Sonar data formats, as high-level Go code, and simple parsers
for the most important of the formats.  Currently there is a definition for only a JSON format
(known as the "new" format since there were older formats than that).

In `kafka-proxy/` is a simple REST proxy for Kafka.  Nodes that are not able to exfiltrate data to a
Kafka broker directly because they are behind an http/https proxy can be configured to use a REST
protocol for exfiltration, and the kafka-proxy component can be configured to receive that traffic
and forward it to the Kafka broker.
