from subprocess import check_output
from collections import defaultdict
import datetime
import time
import socket
import csv
import click


def get_timestamp():
    '''
    Returns time stamp in ISO 8601 with time zone information.
    '''
    # https://stackoverflow.com/a/28147286
    utc_offset_sec = time.altzone if time.localtime().tm_isdst else time.timezone
    utc_offset = datetime.timedelta(seconds=-utc_offset_sec)
    return datetime.datetime.now().replace(tzinfo=datetime.timezone(offset=utc_offset)).isoformat()


def get_slurm_project():
    # FIXME so far not implemented
    return None


def ingore_user(user):
    # FIXME this should be configurable
    system_users = [
        'avahi',
        'colord',
        'dbus',
        'haveged',
        'polkitd',
        'root',
        'rtkit',
        'systemd+',
    ]
    return user in system_users


def extract_processes(text):
    cpu_percentages = defaultdict(float)
    mem_percentages = defaultdict(float)
    for line in text.split('\n'):
        words = line.split()
        if len(words) == 12:
            pid, user, _, _, _, _, _, _, cpu_percentage, mem_percentage, _, command = words
            if not ingore_user(user):
                cpu_percentages[(user, command)] += float(cpu_percentage)
                mem_percentages[(user, command)] += float(mem_percentage)
    return cpu_percentages, mem_percentages


# this is why i like tests close to the implementation
# it helps me to document and understand the function
def test_extract_processes():
    text = '''
    2011 bob        20   0 1741744 169076  86868 S  10.0  20.0   1:07.67 slack
    2022 bob        20   0  247704  44532  31968 S  10.0  15.0   0:00.08 chromium
    2057 bob        20   0  365504 119788  92596 S  10.0  15.0   0:31.22 chromium
    2084 alice      20   0 1392488 419328 167260 S  10.0   5.0   1:25.47 slack
    2087 bob        20   0 1432508 403424 170964 S  10.0   5.0   0:29.67 someapp
    2090 alice      20   0 1399324 413360 172372 S  10.0   5.0   0:55.18 someapp
    2093 alice      20   0 1005680 116708  67912 S  10.0   5.0   0:04.55 someapp
           '''
    cpu_percentages, mem_percentages = extract_processes(text)
    assert cpu_percentages == {('bob', 'slack'): 10.0,
                               ('bob', 'chromium'): 20.0,
                               ('alice', 'slack'): 10.0,
                               ('bob', 'someapp'): 10.0,
                               ('alice', 'someapp'): 20.0}
    assert mem_percentages == {('bob', 'slack'): 20.0,
                               ('bob', 'chromium'): 30.0,
                               ('alice', 'slack'): 5.0,
                               ('bob', 'someapp'): 5.0,
                               ('alice', 'someapp'): 10.0}


@click.command()
@click.option('--output-file', help='Output file.')
def take_snapshot(output_file):

    # -n 1  only one iteration
    # -b    batch mode
    # we also remove the first 7 lines FIXME is this perhaps brittle?
    output = check_output("top -n 1 -b | sed -e '1,7d'", shell=True).decode('utf-8')
    timestamp = get_timestamp()
    hostname = socket.gethostname()
    slurm_project = get_slurm_project()  # FIXME not implemented

    cpu_percentages, mem_percentages = extract_processes(output)

    with open(output_file, mode='w') as f:
        f_writer = csv.writer(f, delimiter='\t', quotechar='"', quoting=csv.QUOTE_MINIMAL)

        for user, command in cpu_percentages:
            cpu_percentage = cpu_percentages[(user, command)]
            mem_percentage = mem_percentages[(user, command)]
            f_writer.writerow([timestamp, hostname, user, slurm_project, command, cpu_percentage, mem_percentage])
