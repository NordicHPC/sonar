# HTTP exfiltration

In the "daemon mode", Sonar stays memory-resident and pushes data to a network sink; one of these
sinks is an HTTP POST endpoint.  See HOWTO-DAEMON.md for general information about this mode and the
options available for configuring the HTTP producer in Sonar.

Data message formats and upload URIs are as described in HOWTO-DAEMON.md.  In brief, the URI is on
the form API-ROOT/TOPIC where the TOPIC is either the plain Sonar data type ("sample", etc) or the
configured topic-prefix catenated with the data-type ("prefix.sample").  The payload is a Sonar JSON
data package, see [NEW-FORMAT.md](NEW-FORMAT.md).

If the API root is configured as an https URL then data are sent over HTTPS, using the standard
certificates for the endpoint host.

The cluster name and upload password are sent along with the POST as an Authentication header, as
for HTTP Basic authentication.
