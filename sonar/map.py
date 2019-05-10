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


def extract_and_map_data(string_map, re_map, input_dir, delimiter, suffix, default_category):

    report = defaultdict(float)
    only_sum = defaultdict(float)

    for filename in glob(os.path.normpath(os.path.join(input_dir, "*" + suffix))):
        with open(filename) as f:
            f_reader = csv.reader(f, delimiter=delimiter, quotechar='"')
            for line in f_reader:
                _, node, num_cores, user, project, job_id, process, cpu_percentage, mem_percentage = tuple(line)
                num_cores = int(num_cores)
                job_id = int(job_id)
                cpu_percentage = float(cpu_percentage)
                mem_percentage = float(mem_percentage)
                app = map_process(process, string_map, re_map, default_category)

                report[(user, project, app)] += cpu_percentage
                only_sum[(app, process)] += cpu_percentage

    return report, only_sum


def output(report, only_sum, default_category):
    percentage_cutoff = 0.5
    print(f'(only contributions above {percentage_cutoff}% shown)')

    cpu_sum = 0.0
    for key in only_sum:
        cpu_sum += only_sum[key]

    only_sum_known = defaultdict(float)
    for app, process in only_sum:
        if app != default_category:
            only_sum_known[app] += only_sum[(app, process)]
    for app in sorted(only_sum_known, key=lambda x: only_sum_known[x], reverse=True):
        percentage = 100.0 * only_sum_known[app] / cpu_sum
        if percentage > percentage_cutoff:
            print(f'- {app:20s} {percentage:6.2f}%')

    cpu_sum_unknown = 0.0
    for app, process in only_sum:
        if app == default_category:
            cpu_sum_unknown += only_sum[(app, process)]
    print(f'\nunknown processes ({100.0*cpu_sum_unknown/cpu_sum:.2f}%):')
    for app, process in sorted(only_sum, key=lambda x: only_sum[x], reverse=True):
        if app == default_category:
            cpu = only_sum[(app, process)]
            percentage = 100.0 * cpu / cpu_sum
            if percentage > percentage_cutoff:
                print(f'- {process:20s} {percentage:6.2f}%')


def main(config):
    """
    Map sonar snap results to a provided list of programs and create an output that is suitable for the dashboard etc.
    """

    string_map, re_map = read_mapping(config["str_map_file"], config["re_map_file"])

    report, only_sum = extract_and_map_data(
        string_map,
        re_map,
        config["input_dir"],
        delimiter=config["input_delimiter"],
        suffix=config["input_suffix"],
        default_category=config["default_category"],
    )

    output(report, only_sum, config["default_category"])

#   let's do file export a bit later
#   first i want to know what data i would like to plot and this will
#   be prototyped using CLI alone
#   later: data export and web

#   f_writer = csv.writer(
#       sys.stdout,
#       delimiter=config["output_delimiter"],
#       quotechar='"',
#       quoting=csv.QUOTE_MINIMAL,
#   )
#   for key in sorted(report, key=lambda x: report[x], reverse=True):
#       user, project, app = key
#       cpu = report[key]
#       f_writer.writerow([user, project, app, "{:.1f}".format(cpu)])
