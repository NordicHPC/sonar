import sys
import os
import re
import csv
import time
import datetime

from glob import glob
from contextlib import contextmanager
from collections import defaultdict
from .version import __version__


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

    unknown_process_cpu_load = defaultdict(float)
    app_cpu_load = defaultdict(float)

    unknown_process_cpu_res = defaultdict(int)
    app_cpu_res = defaultdict(int)

    for filename in glob(os.path.normpath(os.path.join(input_dir, "*" + suffix))):
        with open(filename) as f:
            f_reader = csv.reader(f, delimiter=delimiter, quotechar='"')
            for line in f_reader:

                # The columns are:
                #  0 - time stamp
                #  1 - hostname
                #  2 - number of cores on this node
                #  3 - user
                #  4 - process
                #  5 - CPU percentage (this is a 20-core node)
                #  6 - memory used in MB
                #  7 - Slurm project
                #  8 - Slurm job ID
                #  9 - Number of CPUs requested by the job
                # 10 - Minimum size of memory requested by the job

                num_cores_on_node = int(line[2])
                user = line[3]
                process = line[4]
                cpu_percentage = float(line[5])
                project = line[7]

                app = map_process(process, string_map, re_map, default_category)
                cpu_load = 0.01 * cpu_percentage

                # WARNING: calculation of blocked resources is imprecise if different users or different jobs
                # run on the same node
                if app == default_category:
                    unknown_process_cpu_load[process] += cpu_load
                    unknown_process_cpu_res[(process, user)] += num_cores_on_node
                else:
                    app_cpu_load[app] += cpu_load
                    app_cpu_res[(app, user)] += num_cores_on_node

    return {
        'unknown_process_cpu_load': unknown_process_cpu_load,
        'unknown_process_cpu_res': unknown_process_cpu_res,
        'app_cpu_load': app_cpu_load,
        'app_cpu_res': app_cpu_res,
    }


def _output_section(cpu_load, cpu_load_sum, cpu_res, cpu_res_sum, percentage_cutoff):
    _res = defaultdict(int)
    for key in cpu_res:
        _res[key[0]] += cpu_res[key]
    for key in sorted(_res, key=lambda x: _res[x], reverse=True):
        cpu_load_percentage = 100.0 * cpu_load[key] / cpu_load_sum
        cpu_res_percentage = 100.0 * _res[key] / cpu_res_sum
        if cpu_res_percentage > percentage_cutoff:
            users = {u: cpu_res[(k, u)] for k, u in cpu_res.keys() if k == key}
            top_user = sorted(users, key=lambda x: users[x], reverse=True)[0]
            top_user_res_percentage = 100.0 * cpu_res[(key, top_user)] / cpu_res_sum
            print(f'- {key:16s} {cpu_load_percentage:6.2f}% {cpu_res_percentage:6.2f}%'
                  f'   {top_user} ({top_user_res_percentage:.2f}%)')


def output(data, default_category, percentage_cutoff):

    print(f'sonar v{__version__}')
    print(f'summary generated on {datetime.datetime.now()}')
    print(f'percentage cutoff: {percentage_cutoff}%')
    print()

    print(f'  app                load  reserved  top user')
    print(f'=============================================')

    app_cpu_load_sum = sum(data['app_cpu_load'].values())
    unknown_process_cpu_load_sum = sum(data['unknown_process_cpu_load'].values())
    cpu_load_sum = app_cpu_load_sum + unknown_process_cpu_load_sum

    app_cpu_res_sum = sum(data['app_cpu_res'].values())
    unknown_process_cpu_res_sum = sum(data['unknown_process_cpu_res'].values())
    cpu_res_sum = app_cpu_res_sum + unknown_process_cpu_res_sum

    _output_section(data['app_cpu_load'], cpu_load_sum, data['app_cpu_res'], cpu_res_sum, percentage_cutoff)

    _load_percentage = 100.0 * unknown_process_cpu_load_sum / cpu_load_sum
    _res_percentage = 100.0 * unknown_process_cpu_res_sum / cpu_res_sum

    print()
    print(f'  unmapped         {_load_percentage:6.2f}% {_res_percentage:6.2f}%')
    print(f'----------------------------------')

    _output_section(data['unknown_process_cpu_load'], cpu_load_sum, data['unknown_process_cpu_res'], cpu_res_sum, percentage_cutoff)


def main(config):
    """
    Map sonar snap results to a provided list of programs and create an output that is suitable for the dashboard etc.
    """

    string_map, re_map = read_mapping(config["str_map_file"], config["re_map_file"])

    data = extract_and_map_data(
        string_map,
        re_map,
        config["input_dir"],
        delimiter=config["input_delimiter"],
        suffix=config["input_suffix"],
        default_category=config["default_category"],
    )

    output(data, config["default_category"], config["percentage_cutoff"])

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
