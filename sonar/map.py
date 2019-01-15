#!/usr/bin/env python3

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
    '''
    Reads string_map_file and re_map_file unless they are None.
    Retuns string_map as a dictionary and returns re_map as a list of tuples.
    '''

    string_map = []
    re_map = []

    for file_name, l in [(string_map_file, string_map),
                         (re_map_file, re_map)]:
        if file_name:
            try:
                with open(file_name) as f:
                    f_reader = csv.reader(f, delimiter='\t', quotechar='"')
                    for k, v in f_reader:
                        l.append((k, v))
            except FileNotFoundError:
                print('ERROR: file "{0}" not found.'.format(file_name), file=sys.stderr)

    return {'string': dict(string_map), 're': re_map}


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
def map_app(string, string_map, re_map, default_category='UNKNOWN'):
    '''
    Map the `string` using string_map and re_map.
    Returns the app or `default_category` if the appstring could not be identified.
    '''

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
        ('^skypeforlinux$', 'Skype'),
        ('^firefox$', 'Firefox'),
        ('[a-z][A-Z][0-9]', 'MyFancyApp'),
        ('^firefox$', 'NOTFirefox')
    ]

    assert map_app('asf', {}, re_map) == 'UNKNOWN'
    assert map_app('firefox', {}, re_map) == 'Firefox'
    assert map_app('aaaxY9zzz', {}, re_map) == 'MyFancyApp'

    # test the cache
    assert map_app('firefox', {}, re_map=[('^firefox$', 'redefined')]) == 'Firefox'


def create_report(mapping, input_dir, start, end, delimiter, suffix='.tsv', default_category='UNKNOWN'):

    # FIXME: This should be split into two functions, one reading the files, the other doing the actual parsing for better testing.

    # Add the local timezone to start and end
    utc_offset_sec = time.altzone if time.localtime().tm_isdst else time.timezone
    utc_offset = datetime.timedelta(seconds=-utc_offset_sec)
    start = start.replace(tzinfo=datetime.timezone(offset=utc_offset))
    end = end.replace(tzinfo=datetime.timezone(offset=utc_offset))

    report = defaultdict(float)

    for filename in glob(os.path.normpath(os.path.join(input_dir, '*' + suffix))):
        with open(filename) as f:
            f_reader = csv.reader(f, delimiter=delimiter, quotechar='"')
            for line in f_reader:
                date = datetime.datetime.strptime(line[0], '%Y-%m-%dT%H:%M:%S.%f%z')
                if date < start:
                    continue
                if date > end:
                    break

                user = line[2]
                project = line[3]
                jobid = line[4]
                app = map_app(line[5], mapping['string'], mapping['re'], default_category)
                cpu = float(line[6])

                report[(user, project, app)] += cpu

    return report


@contextmanager
def write_open(filename, suffix):
    '''
    Special wrapper to allow to write to stdout or a file nicely. If `filename` is '-' or None, everything will be written to stdout instead to a "real" file.

    Use like:
    >>> with write_open('myfile') as f:
    >>>     f.write(...)
    or
    >>> with write_open() as f:
    >>>     f.write(...)
    '''

    # https://stackoverflow.com/q/17602878
    if filename and filename != '-':
        if not filename.endswith(suffix):
            filename += suffix
        handler = open(filename, 'a')
    else:
        handler = sys.stdout

    try:
        yield handler
    finally:
        if handler is not sys.stdout:
            handler.close()


def do_mapping(config):
    '''
    Map sonar snap results to a provided list of programs and create an output that is suitable for the dashboard etc.
    '''

    mapping = read_mapping(config['str_map_file'], config['re_map_file'])

    today = datetime.datetime.now()
    yesterday = today - datetime.timedelta(days=1)

    report = create_report(mapping, config['input_dir'], start=yesterday, end=today, delimiter=config['snap_delimiter'], suffix=config['snap_suffix'], default_category=config['default_category'])

    with write_open(config['output_file'], config['map_suffix']) as f:
        f_writer = csv.writer(f, delimiter=config['map_delimiter'], quotechar='"', quoting=csv.QUOTE_MINIMAL)
        for key in sorted(report, key=lambda x: report[x], reverse=True):
            user, project, app = key
            cpu = report[key]
            f_writer.writerow([user, project, app, '{:.1f}'.format(cpu)])
