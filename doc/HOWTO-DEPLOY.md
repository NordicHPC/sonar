# Deploying Sonar

As per [the user manual](MANUAL.md), Sonar is normally set up in a configuration in which it
collects different kinds of information at different cadences: process sampling (`sonar sample`)
every few minutes; slurm information (when applicable) a couple times per hour (`sonar jobs` and
`sonar cluster`); system information every day (`sonar sysinfo`).  The data are then either saved in
a directory tree or exfiltrated to an off-node back-end.

Most typically on an HPC cluster the data are sent off-node with some randomization to avoid data
storms on the shared disk or on the network.  This exfiltration will typically use Sonar's built-in
Kafka exfiltration path.  Alternatively, it can take the form of a script that captures Sonar output
and forwards it over HTTPS with curl.

The collecting of information is usually driven by Sonar's built-in daemon mode in which a config
file describes cadences, exfiltration paths, authentication information, and other things, along
with a systemd service to control the daemon.  Alternatively, it can take the form of cron jobs that
run Sonar in one-shot mode and captures stdout output.

TODO: the rest of this to be written for #493.

The most complete and current information is probably the description of how Sonar is deployed on
NRIS systems, currently available only to NRIS members in [a Sigma2 gitlab
repo](https://gitlab.sigma2.no/larstha/sonar-deploy); ideally this will be opened up eventually.
Somewhat older but still correct information can be found in the "production" subdirectory of the
[Jobanalyzer](https://github.com/NAICNO/Jobanalyzer) repo.

Also take a look at the [HOWTO-DAEMON.md](HOWTO-DAEMON.md) and [HOWTO-KAFKA.md](HOWTO-KAFKA.md) files
in this directory, both of which describe many details of a possible deployment.
