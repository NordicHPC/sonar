# Deploying Sonar

## Context

As described in [the user manual](MANUAL.md), Sonar is normally set up in a configuration in which
it collects different kinds of information at different cadences: process sampling (`sonar sample`)
every few minutes; Slurm information (when applicable) a couple times per hour (`sonar jobs`) or a
few times per day (`sonar cluster`); node configuration information every day (`sonar sysinfo`).
The data are then either saved in a directory tree or exfiltrated to an off-node back-end.

Most typically on an HPC cluster the data are sent off-node with some randomization to avoid data
storms on the shared disk or on the network.  This exfiltration will typically use Sonar's built-in
Kafka exfiltration path.  (Alternatively, it can take the form of a script that captures Sonar
output and forwards it over HTTPS with curl.)

The collecting of information is usually driven by Sonar's built-in daemon mode in which a config
file describes cadences, exfiltration paths, authentication information, and other things, along
with a systemd service to control the daemon.  (Alternatively, it can take the form of cron jobs
that run Sonar in one-shot mode and captures stdout output.)

Again, the user manual describes how to write configuration files, generate crypto materials, and so
on.

## Sonar running under systemd

Sonar can be installed from RPMs into system directories so that it can be run under systemd.  See
[build-dist/README.md](../build-dist/README.md) for instructions on building the RPMs.

A typical installation has a user `sonar` in a group `sonar` and installs the Sonar files in
`/usr/local/lib/sonar`.  In that location are the Sonar binary, the Sonar config file, an maybe a
subdirectory called `secrets` with the password file and the TLS server certificate for
communicating with the broker.  In addition, there is a systemd service file.  The service file used
by default for RPM builds is in
[build-dist/rpm-assets/sonar.service](../build-dist/rpm-assets/sonar.service).

NOTE!  By default, the service file requires user/group `sonar/sonar`, which must exist before
starting the service.  See below.

### SELinux

If SELinux is enabled then, as I understand it, the files *must* be in `/usr/local/lib` (on recent
systems).

Another SELinux matter is the User and Group settings for the service.  Under some configurations,
at least (RHEL8 but not FC43), the User and Group directives in the service file won't be honored
and a workaround may be needed.

Modify the `[Service]` section of the service file from this:

```
User=sonar
Group=sonar
ExecStart=/usr/local/lib/sonar/sonar daemon /usr/local/lib/sonar/sonar.cfg
```

to this, working around the problem:

```
ExecStart=/usr/sbin/runuser -u sonar -- \
    /usr/local/lib/sonar/sonar daemon /usr/local/lib/sonar/sonar.cfg
```

### Secrets

A typical install will have a subdirectory `/usr/local/lib/sonar/secrets` owned by `sonar/sonar` and
*with restrictive permissions*.  In that directory are typically the upload password file (see
[MANUAL.md](MANUAL.md#shared-kafka-setings)) and optionally the certificate for communicating with
the broker.  These are not created by the default RPM installation but have to be managed manually.

Thus the Sonar configuration ends up looking like this, for example:

```
[kafka]
broker-address = my-aggregator.uio.no:1234
ca-file = /usr/local/lib/sonar/secrets/my-aggregator-ca.crt
sasl-password-file = /usr/local/lib/sonar/secrets/my-aggregator-upload-password.txt
```

## Back-end

The specific back-ends have their own instructions for how they are set up but there are
commonalities.

A typical back-end has a Kafka broker that Sonar can send data to, and a database manager that
ingests data from the broker.  The database manager is out of scope for this document.

The broker will receive messages to specific topics (see [Kafka topics](MANUAL.md#kafka-topics) in
the manual), which it must allow.  If Sonar is configured with an upload password per cluster then
the broker must allow traffic from the cluster-name/upload-password combination, ideally only to
topics matching the cluster-name.  If Sonar is communicating directly with the broker then that
communication is likely over TLS; the broker must use the same TLS certificate that Sonar uses.

If the nodes running Sonar are behind an HTTP proxy on their cluster then the back-end must either
open outgoing ports for the Kafka traffic, or it must also run the Kafka HTTP proxy that allows
Sonar to communicate with the broker, as Kafka cannot use HTTP(S).

A Kafka HTTP proxy implementation that Sonar can communicate with is in
[util/kafka-proxy/kprox.go](../util/kafka-proxy/kprox.go), see that file for proxy configuration
instructions.  In this scenario, Sonar will still be configured with the upload password, but it
will communicate via HTTP POST and it is the Kafka HTTP proxy that must be configured with the Kafka
broker address and TLS certificate, if necessary.

## Additional information (NRIS)

There are current, adapted install artifacts for Sonar on NRIS systems in [a Sigma2 gitlab
repo](https://gitlab.sigma2.no/larstha/sonar-deploy).  Currently this is available only to NRIS
members.  Ideally it will be opened up eventually.

## Users and groups

To add the group:

```
sudo groupadd sonar
```

To add the user with a home directory matching the default install location:

```
sudo useradd -r -g sonar -M -d /usr/local/lib/sonar -s /sbin/nologin -c "Sonar profiling daemon" sonar
```

