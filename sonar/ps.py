import sys
import datetime
import csv
import multiprocessing
import psutil
import platform
import click
from collections import defaultdict

from sonar.command import safe_command


def extract_processes(raw_text: str):
    """
    Extract user, cpu, memory, and command from `raw_text` that should be the (special) output of a `ps` command.
    """
    cpu_percentages = defaultdict(float)
    mem_percentages = defaultdict(float)

    for line in raw_text.splitlines():
        # Using maxsplit to prevent commands to be split. This is unstable if the `ps` call is altered!
        words = line.split(maxsplit=4)
        _pid, user, cpu_percentage, mem_percentage, command = words
        cpu_percentages[(user, command)] += float(cpu_percentage)
        mem_percentages[(user, command)] += float(mem_percentage)

    return cpu_percentages, mem_percentages


def test_extract_processes():
    text = """2011 bob                    10.0  20.0   slack
     2022 bob                    10.0  15.0   chromium
    12057 bob                    10.0  15.0   chromium
     2084 alice                  10.0   5.0   slack
     2087 bob                    10.0   5.0   someapp
     2090 alice                  10.0   5.0   someapp
     2093 alice                  10.0   5.0   someapp"""

    cpu_percentages, mem_percentages = extract_processes(text)

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


def create_snapshot(cpu_cutoff_percent: float, mem_cutoff_percent: float):
    """
    Take a snapshot of the currently running processes that use more than
    `cpu_cutoff_percent` cpu and `mem_cutoff_percent` memory, ignoring the set
    or list `ignored_users`. Returns a list of listsC. being lines of columns.
    """

    # -e      show all processes
    # -o      output formatting. user:30 is a hack to prevent cut-off user names
    command = "ps -e --no-header -o pid,user:30,pcpu,pmem,comm"

    output = safe_command(command=command, timeout_seconds=2)

    cpu_percentages, mem_percentages = extract_processes(output)

    timestamp = datetime.datetime.now().astimezone().isoformat()
    hostname = platform.node()
    available_memory_bytes = psutil.virtual_memory().total
    num_cores = multiprocessing.cpu_count()

    snapshot = []

    for (user, command), cpu_percentage in cpu_percentages.items():
        if cpu_percentage >= cpu_cutoff_percent:
            mem_percentage = mem_percentages[(user, command)]
            if mem_percentage >= mem_cutoff_percent:
                memory_mib = int(
                    available_memory_bytes * mem_percentage / 104857600
                )  # 1024*1024*100
                snapshot.append(
                    [
                        timestamp,
                        hostname,
                        num_cores,
                        user,
                        command,
                        "{:.1f}".format(cpu_percentage),
                        memory_mib,
                    ]
                )

    return snapshot


@click.command()
@click.option(
    "--cpu-cutoff-percent", default=0.5, help="CPU consumption percentage cutoff."
)
@click.option(
    "--mem-cutoff-percent", default=0.5, help="Memory consumption percentage cutoff."
)
def print_ps_info(cpu_cutoff_percent, mem_cutoff_percent):
    """
    Take a snapshot of the currently running processes that use more than
    `cpu_cutoff_percent` cpu and `mem_cutoff_percent` memory and print it to stdout.
    """
    snapshot = create_snapshot(cpu_cutoff_percent, mem_cutoff_percent)

    writer = csv.writer(
        sys.stdout,
        quotechar='"',
        quoting=csv.QUOTE_MINIMAL,
    )
    writer.writerows(snapshot)


if __name__ == "__main__":
    print_ps_info()
