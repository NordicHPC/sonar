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

    return dict(string_map), re_map


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
def map_process(process, string_map, re_map, default_category):
    """
    Map the process using string_map and re_map.
    Returns the app or `default_category` if the process could not be identified.
    """

    try:
        return string_map[process]
    except KeyError:
        pass

    for k, v in re_map:
        if re.search(k, process) is not None:
            return v

    return default_category


def test_map_process():
    # FIXME: This needs more tests including string_map
    re_map = [
        ("^skypeforlinux$", "Skype"),
        ("^firefox$", "Firefox"),
        ("[a-z][A-Z][0-9]", "MyFancyApp"),
        ("^firefox$", "NOTFirefox"),
    ]

    assert map_process("asf", {}, re_map, "unknown") == "unknown"
    assert map_process("firefox", {}, re_map, "") == "Firefox"
    assert map_process("aaaxY9zzz", {}, re_map, "") == "MyFancyApp"

    # test the cache
    assert map_process("firefox", {}, re_map=[("^firefox$", "redefined")]) == "Firefox"


def create_report(string_map, re_map, input_dir, delimiter, suffix, default_category):

    # FIXME: This should be split into two functions, one reading the files, the other doing the actual parsing for better testing.

    report = defaultdict(float)
    only_sum = defaultdict(float)

    for filename in glob(os.path.normpath(os.path.join(input_dir, "*" + suffix))):
        with open(filename) as f:
            f_reader = csv.reader(f, delimiter=delimiter, quotechar='"')
            for line in f_reader:
                user = line[2]
                project = line[3]
                jobid = line[4]
                process = line[-3]
                app = map_process(process, string_map, re_map, default_category)
                cpu = float(line[6])

                report[(user, project, app)] += cpu
                only_sum[(app, process)] += cpu

    return report, only_sum


def main(config):
    """
    Map sonar snap results to a provided list of programs and create an output that is suitable for the dashboard etc.
    """

    string_map, re_map = read_mapping(config["str_map_file"], config["re_map_file"])

    report, only_sum = create_report(
        string_map,
        re_map,
        config["input_dir"],
        delimiter=config["input_delimiter"],
        suffix=config["input_suffix"],
        default_category=config["default_category"],
    )

    if config["only_check_mapping"]:

        cpu_sum = 0.0
        for key in only_sum:
            cpu_sum += only_sum[key]

        cpu_sum_unknown = 0.0
        for key in only_sum:
            if key[0] == config["default_category"]:
                cpu_sum_unknown += only_sum[key]

        for app, process in sorted(only_sum, key=lambda x: only_sum[x], reverse=True):
            cpu = only_sum[(app, process)]
            if app != config["default_category"]:
                print(f'- {app:20s} {process:20s} {100.0*cpu/cpu_sum:6.2f}%')

        print(f'\nunknown processes ({100.0*cpu_sum_unknown/cpu_sum:.2f}%):')
        print(f'(only contributions above 0.1% shown)')
        for app, process in sorted(only_sum, key=lambda x: only_sum[x], reverse=True):
            cpu = only_sum[(app, process)]
            percentage = 100.0*cpu/cpu_sum
            if app == config["default_category"]:
                if percentage > 0.1:
                    print(f'- {process:20s} {percentage:6.2f}%')
        return

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
