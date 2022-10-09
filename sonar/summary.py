import sys
import re
import csv
import datetime
import click

from pathlib import Path
from tqdm import tqdm
from collections import defaultdict
from tabulate import tabulate


def read_mapping(file_name: str):
    """
    Retuns a list of tuples.
    """
    l = []

    try:
        with open(file_name) as f:
            f_reader = csv.reader(
                f, skipinitialspace=True, delimiter=" ", quotechar='"'
            )
            for k, v in f_reader:
                l.append((k, v))
    except FileNotFoundError:
        print('ERROR: file "{0}" not found.'.format(file_name), file=sys.stderr)

    return l


def memoize_on_first_arg(func):
    cache = dict()

    def memoized_func(*args, **kwargs):
        string = args[0]
        if string in cache:
            return cache[string]
        result = func(*args, **kwargs)
        cache[string] = result
        return result

    return memoized_func


@memoize_on_first_arg
def map_process(process, string_map, regex_map):
    """
    Map the process using string_map and regex_map. Returns the app or
    unknown-{process} if the process could not be identified.
    """

    try:
        return string_map[process]
    except KeyError:
        pass

    for k, v in regex_map:
        if re.search(k, process) is not None:
            return v

    return f"unknown-{process}"


def test_map_process():
    regex_map = [
        ("^skypeforlinux$", "Skype"),
        ("^firefox$", "Firefox"),
        ("[a-z][A-Z][0-9]", "MyFancyApp"),
        ("^firefox$", "NOTFirefox"),
    ]

    assert map_process("asf", {}, regex_map) == "unknown-asf"
    assert map_process("firefox", {}, regex_map) == "Firefox"
    assert map_process("aaaxY9zzz", {}, regex_map) == "MyFancyApp"

    # test the cache
    assert (
        map_process("firefox", {}, regex_map=[("^firefox$", "redefined")]) == "Firefox"
    )


def normalize_time_stamp(time_stamp):
    return time_stamp.split("T")[0]


def test_normalize_time_stamp():
    assert normalize_time_stamp("2022-10-09T15:05:18.209288+02:00") == "2022-10-09"


def extract_and_map_data(string_map, regex_map, input_dir, start_date, end_date):
    _start_date = datetime.datetime.strptime(start_date, "%Y-%m-%d")
    _end_date = datetime.datetime.strptime(end_date, "%Y-%m-%d")

    cpu_load = defaultdict(float)

    files = Path(input_dir).glob("**/*.csv")
    print("reading log files ...")
    for filename in tqdm(files):
        with open(filename) as f:
            f_reader = csv.reader(f, quotechar='"')
            for line in f_reader:
                # The columns are:
                #  0 - time stamp
                #  1 - hostname
                #  2 - number of cores on this node
                #  3 - user
                #  4 - process
                #  5 - CPU percentage
                #  6 - memory used in MB

                time_stamp = datetime.datetime.strptime(
                    normalize_time_stamp(line[0]), "%Y-%m-%d"
                )
                if _start_date <= time_stamp <= _end_date:

                    user = line[3]
                    process = line[4]
                    app = map_process(process, string_map, regex_map)
                    cpu_percentage = float(line[5])

                    cpu_load[(app, user)] += 0.01 * cpu_percentage

    return cpu_load


def today():
    return datetime.datetime.today()


def two_weeks_ago():
    return today() - datetime.timedelta(days=14)


@click.command("cli", context_settings={"show_default": True})
@click.option(
    "--input-dir",
    required=True,
    type=str,
    help="Input directory that holds the snapshots collected with 'sonar ps'.",
)
@click.option(
    "--start-date",
    default=two_weeks_ago().strftime("%Y-%m-%d"),
    help="Start date.",
)
@click.option(
    "--end-date",
    default=today().strftime("%Y-%m-%d"),
    help="End date.",
)
@click.option("--percentage-cutoff", default=2.0, help="Percentage cutoff.")
@click.option(
    "--str-map-file",
    default=None,
    type=str,
    help="File with the string mapping information (process -> application) [default: use internal mapping file].",
)
@click.option(
    "--regex-map-file",
    default=None,
    type=str,
    help="File with the regular expression mapping information (process -> application) [default: use internal mapping file].",
)
def generate_summary(
    input_dir, start_date, end_date, percentage_cutoff, str_map_file, regex_map_file
):

    script_dir = Path(__file__).resolve().parent
    if str_map_file is None:
        str_map_file = Path(f"{script_dir}/mapping/string_map.txt")
    if regex_map_file is None:
        regex_map_file = Path(f"{script_dir}/mapping/regex_map.txt")

    string_map = dict(read_mapping(str_map_file))
    regex_map = read_mapping(regex_map_file)

    data = extract_and_map_data(string_map, regex_map, input_dir, start_date, end_date)

    total_load = 0.0
    app_load = defaultdict(float)
    for ((app, _user), f) in data.items():
        app_load[app] += f
        total_load += f

    table = []
    table.append(["app", "percentage"])
    for (app, load) in sorted(app_load.items(), key=lambda t: t[1], reverse=True):
        percentage = 100.0 * load / total_load
        if percentage > percentage_cutoff:
            table.append([app, percentage])

    print(tabulate(table, headers="firstrow"))
