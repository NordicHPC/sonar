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
        if file_name is not None:
            try:
                with open(file_name) as f:
                    f_reader = csv.reader(f, delimiter='\t', quotechar='"')
                    for k, v in f_reader:
                        l.append((k, v))
            except FileNotFoundError:
                sys.stderr.write(f'ERROR: file {file_name} not found\n')

    return {'string': dict(string_map), 're': re_map}


# Please note that map_cache is persistent between calls and should not be given as argument.
def map_app(string, string_map, re_map, map_cache={}):
    '''
    Map the `string` using string_map and re_map.
    Never define `map_cache`!
    Returns the app or "UNKNOWN" if the appstring could not be identified.
    '''

    try:
        return map_cache[string]
    except KeyError:
        pass

    try:
        return string_map[string]
    except KeyError:
        pass

    for k, v in re_map:
        if re.search(k, string) is not None:
            return v

    return 'UNKNOWN'


def test_map_app():
    # FIXME: This needs more tests including string_map and maybe even test the cache
    re_map = [
        ('^skypeforlinux$', 'Skype'),
        ('^firefox$', 'Firefox'),
        ('[a-z][A-Z][0-9]', 'MyFancyApp'),
        ('^firefox$', 'NOTFirefox')
    ]

    assert map_app('asf', {}, re_map) == 'UNKNOWN'
    assert map_app('firefox', {}, re_map) == 'Firefox'
    assert map_app('aaaxY9zzz', {}, re_map) == 'MyFancyApp'

    # here test the cache
#   assert map_app('firefox', {}, re_map=[('^firefox$', 'redefined')]) == 'Firefox'


def create_report(mapping, snap_dir, start, end, suffix='.tsv'):

    # FIXME: This should be split into two functions, one reading the files, the other doing the actual parsing for better testing.
    # FIXME: The suffix should be readable from config

    # Add the local timezone to start and end
    utc_offset_sec = time.altzone if time.localtime().tm_isdst else time.timezone
    utc_offset = datetime.timedelta(seconds=-utc_offset_sec)
    start = start.replace(tzinfo=datetime.timezone(offset=utc_offset))
    end = end.replace(tzinfo=datetime.timezone(offset=utc_offset))

    report = defaultdict(float)

    for filename in glob(os.path.normpath(os.path.join(snap_dir, '*' + suffix))):
        with open(filename) as f:
            f_reader = csv.reader(f, delimiter='\t', quotechar='"')
            for line in f_reader:
                date = datetime.datetime.strptime(line[0], '%Y-%m-%dT%H:%M:%S.%f%z')
                if date < start:
                    continue
                if date > end:
                    break

                user = line[2]
                project = line[3]
                app = map_app(line[4], mapping['string'], mapping['re'])
                cpu = float(line[5])

                report[(user, project, app)] += cpu

    return report


@contextmanager
def write_open(filename=None):
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
        handler = open(filename, 'w')
    else:
        handler = sys.stdout

    try:
        yield handler
    finally:
        if handler is not sys.stdout:
            handler.close()


def do_mapping(output_file, string_map_file, re_map_file, snap_dir):
    '''
    Map sonar snap results to a provided list of programs and create an output that is suitable for the dashboard etc.
    '''

    mapping = read_mapping(string_map_file, re_map_file)

    today = datetime.datetime.now()
    yesterday = today - datetime.timedelta(days=1)

    report = create_report(mapping, snap_dir, start=yesterday, end=today)

    with write_open(output_file) as f:
        f_writer = csv.writer(f, delimiter='\t', quotechar='"', quoting=csv.QUOTE_MINIMAL)
        for key in sorted(report, key=lambda x: report[x], reverse=True):
            user, project, app = key
            cpu = report[key]
            f_writer.writerow([user, project, app, '{:.1f}'.format(cpu)])
