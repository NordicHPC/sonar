import sys
import os
import re
import csv
import time
import datetime

from glob import glob
from contextlib import contextmanager
from collections import defaultdict


def read_mapping(string_map_file, re_map_file):
    """
    Reads string_map_file and re_map_file unless they are falsy.
    Retuns a dictionary with string_map as a dictionary and re_map as a list of tuples.
    """

    string_map = []
    re_map = []

    for file_name, l in [(string_map_file, string_map), (re_map_file, re_map)]:
        if file_name:
            try:
                with open(file_name) as f:
                    f_reader = csv.reader(f, skipinitialspace=True, delimiter=" ", quotechar='"')
                    for k, v in f_reader:
                        l.append((k, v))
            except FileNotFoundError:
                print('ERROR: file "{0}" not found.'.format(file_name), file=sys.stderr)

    return {"string": dict(string_map), "re": re_map}


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
def map_app(string, string_map, re_map, default_category):
    """
    Map the `string` using string_map and re_map.
    Returns the app or `default_category` if the appstring could not be identified.
    """

    try:
        return string_map[string]
    except KeyError:
        pass

    for k, v in re_map:
        if re.search(k, string) is not None:
            return v

    return default_category


def test_map_app():
    # FIXME: This needs more tests including string_map
    re_map = [
        ("^skypeforlinux$", "Skype"),
        ("^firefox$", "Firefox"),
        ("[a-z][A-Z][0-9]", "MyFancyApp"),
        ("^firefox$", "NOTFirefox"),
    ]

    assert map_app("asf", {}, re_map, "unknown") == "unknown"
    assert map_app("firefox", {}, re_map, "") == "Firefox"
    assert map_app("aaaxY9zzz", {}, re_map, "") == "MyFancyApp"

    # test the cache
    assert map_app("firefox", {}, re_map=[("^firefox$", "redefined")]) == "Firefox"


def _normalize_date(date):
    _intermediate = datetime.datetime.strftime(date, "%Y-%m-%d")
    date_normalized = datetime.datetime.strptime(_intermediate, "%Y-%m-%d")
    return date_normalized


def create_report(mapping, input_dir, start, end, delimiter, suffix, default_category):

    # FIXME: This should be split into two functions, one reading the files, the other doing the actual parsing for better testing.

    start_normalized = _normalize_date(start)
    end_normalized = _normalize_date(end)

    report = defaultdict(float)

    mapping_dict = {}

    for filename in glob(os.path.normpath(os.path.join(input_dir, "*" + suffix))):
        with open(filename) as f:
            f_reader = csv.reader(f, delimiter=delimiter, quotechar='"')
            for line in f_reader:
                if line[5] not in mapping_dict:
                    app = map_app(
                        line[5], mapping["string"], mapping["re"], default_category
                    )
                    mapping_dict[line[5]] = app

                date_normalized = _normalize_date(
                    datetime.datetime.strptime(line[0], "%Y-%m-%dT%H:%M:%S.%f%z")
                )
                if date_normalized < start_normalized:
                    continue
                if date_normalized > end_normalized:
                    break

                user = line[2]
                project = line[3]
                jobid = line[4]
                app = map_app(
                    line[5], mapping["string"], mapping["re"], default_category
                )
                cpu = float(line[6])

                report[(user, project, app)] += cpu

    return report


def main(config):
    """
    Map sonar snap results to a provided list of programs and create an output that is suitable for the dashboard etc.
    """

    mapping = read_mapping(config["str_map_file"], config["re_map_file"])

    start = datetime.datetime.strptime(config["start_date"], "%Y-%m-%d")
    end = datetime.datetime.strptime(config["end_date"], "%Y-%m-%d")

    report = create_report(
        mapping,
        config["input_dir"],
        start=start,
        end=end,
        delimiter=config["input_delimiter"],
        suffix=config["input_suffix"],
        default_category=config["default_category"],
    )

    f_writer = csv.writer(
        sys.stdout,
        delimiter=config["output_delimiter"],
        quotechar='"',
        quoting=csv.QUOTE_MINIMAL,
    )
    for key in sorted(report, key=lambda x: report[x], reverse=True):
        user, project, app = key
        cpu = report[key]
        f_writer.writerow([user, project, app, "{:.1f}".format(cpu)])
