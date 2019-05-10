import sys
import os
import datetime
import time
import csv
import socket
import multiprocessing

from contextlib import contextmanager
from subprocess import check_output, SubprocessError, DEVNULL
from collections import defaultdict, namedtuple
from configparser import ConfigParser


def get_timestamp():
    """
    Returns time stamp as string in ISO 8601 with time zone information.
    """

    # https://stackoverflow.com/a/28147286
    utc_offset_sec = time.altzone if time.localtime().tm_isdst else time.timezone
    utc_offset = datetime.timedelta(seconds=-utc_offset_sec)

    return (
        datetime.datetime.now()
        .replace(tzinfo=datetime.timezone(offset=utc_offset))
        .strftime("%Y-%m-%dT%H:%M:%S.%f%z")
    )


def get_slurm_info(hostname):
    """
    Try to get the users, jobids, and projects from the current `hostname`.
    If a user should run two jobs with two different projects or jobids, only the last discovered values will be assumed for the user.

    :returns: A defaultdict with the mapping from user to project. Project is '-' if the user is not found or slurm is not available.
    """

    user_to_slurminfo = defaultdict(lambda: {"jobid": "-", "project": "-", "num_cores": "-", "min_mem": "-"})

    # %i  Job ID (or <jobid>_<arrayid> for job arrays)
    # %a  Account (project)
    # %u  User
    try:
        command = f"squeue --noheader --nodelist={hostname} --format=%i,%a,%u,%m,%C"
        output = check_output(command, shell=True, stderr=DEVNULL).decode("utf8")
    except SubprocessError:
        # if Slurm is not available, return the empty defaultdict that will return '-' for any key call.
        return user_to_slurminfo

    for line in output.split("\n"):
        line = line.strip()
        if not line:
            continue
        jobid, project, user, min_mem, num_cores = line.split(",")
        user_to_slurminfo[user] = {"jobid": jobid, "project": project, "num_cores": num_cores, "min_mem": min_mem}

    return user_to_slurminfo


def get_available_memory():
    """
    Tries to return the memory available on the current node in bytes. Returns a negative number if the value cannot be determined.
    This is Unix-specific.
    """

    # Another possibility would be to read /proc/meminfo
    return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")


def extract_processes(raw_text, ignored_users):
    """
    Extract user, cpu, memory, and command from `raw_text` that should be the (special) output of a `ps` command.
    `ignored_users` should be a list with users that shall be ignored.
    """

    cpu_percentages = defaultdict(float)
    mem_percentages = defaultdict(float)
    for line in raw_text.split("\n"):
        # Using maxsplit to prevent commands to be split. This is unstable if the `ps` call is altered!
        words = line.split(maxsplit=4)
        if len(words) == 5:
            pid, user, cpu_percentage, mem_percentage, command = words
            if user not in ignored_users:
                cpu_percentages[(user, command)] += float(cpu_percentage)
                mem_percentages[(user, command)] += float(mem_percentage)

    return cpu_percentages, mem_percentages


def test_extract_processes():
    text = """
     2011 bob                    10.0  20.0   slack
     2022 bob                    10.0  15.0   chromium
    12057 bob                    10.0  15.0   chromium
     2084 alice                  10.0   5.0   slack
     2087 bob                    10.0   5.0   someapp
     2090 alice                  10.0   5.0   someapp
     2093 alice                  10.0   5.0   someapp
    """

    cpu_percentages, mem_percentages = extract_processes(
        raw_text=text, ignored_users=[]
    )

    assert cpu_percentages == {
        ("bob", "slack"): 10.0,
        ("bob", "chromium"): 20.0,
        ("alice", "slack"): 10.0,
        ("bob", "someapp"): 10.0,
        ("alice", "someapp"): 20.0,
    }
    assert mem_percentages == {
        ("bob", "slack"): 20.0,
        ("bob", "chromium"): 30.0,
        ("alice", "slack"): 5.0,
        ("bob", "someapp"): 5.0,
        ("alice", "someapp"): 10.0,
    }

    cpu_percentages, mem_percentages = extract_processes(
        raw_text=text, ignored_users=["bob"]
    )

    assert cpu_percentages == {("alice", "slack"): 10.0, ("alice", "someapp"): 20.0}
    assert mem_percentages == {("alice", "slack"): 5.0, ("alice", "someapp"): 10.0}


def get_hostname():
    # we first try to get the hostname alias
    # we do this because at least on our cluster slurm uses
    # the alias ("c61-8") instead of the full hostname (e.g. "c61-8.local")
    result = check_output(["hostname", "-a"]).rstrip().decode("utf-8")
    if result != "":
        return result
    else:
        # if alias is empty, we try hostname
        return socket.gethostname()


def create_snapshot(cpu_cutoff, mem_cutoff, ignored_users):
    """
    Take a snapshot of the currently running processes that use more than `cpu_cutoff` cpu and `mem_cutoff` memory, ignoring the set or list `ignored_users`. Return a list of lists being lines of columns.
    """

    # -e      show all processes
    # -o      output formatting. user:30 is a hack to prevent cut-off user names
    output = check_output(
        "ps -e --no-header -o pid,user:30,pcpu,pmem,comm", shell=True
    ).decode("utf-8")
    timestamp = get_timestamp()
    hostname = get_hostname()
    num_cores = multiprocessing.cpu_count()
    slurm_info = get_slurm_info(hostname)
    total_memory = get_available_memory()
    if total_memory < 0:
        total_memory = 1

    cpu_percentages, mem_percentages = extract_processes(
        raw_text=output, ignored_users=ignored_users
    )

    snapshot = []

    for user, command in cpu_percentages:
        cpu_percentage = cpu_percentages[(user, command)]
        if cpu_percentage >= cpu_cutoff:
            mem_percentage = mem_percentages[(user, command)]
            if mem_percentage >= mem_cutoff:
                # Weird number is 1024*1024*100 to get MiB and %
                mem_absolute = int(total_memory * mem_percentage / 104857600)
                snapshot.append(
                    [
                        timestamp,
                        hostname,
                        num_cores,
                        user,
                        command,
                        "{:.1f}".format(cpu_percentage),
                        mem_absolute,
                        slurm_info[user]["project"],
                        slurm_info[user]["jobid"],
                        slurm_info[user]["num_cores"],
                        slurm_info[user]["min_mem"],
                    ]
                )

    return snapshot


def main(config):
    """
    Take a snapshot of the currently running processes that use more than `cpu_cutoff` cpu and `mem_cutoff` memory and print it to stdout.
    """

    snapshot = create_snapshot(
        config["cpu_cutoff"], config["mem_cutoff"], config["ignored_users"]
    )

    f_writer = csv.writer(
        sys.stdout,
        delimiter=config["output_delimiter"],
        quotechar='"',
        quoting=csv.QUOTE_MINIMAL,
    )
    f_writer.writerows(snapshot)
