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

import click


def read_mapping(map_file):
    '''
    Read and return a string-to-app mapping (dict) from the tsv filename `map_file`.
    '''

    mapping = []
    with open(map_file) as f:
        f_reader = csv.reader(f, delimiter='\t', quotechar='"')
        for line in f_reader:
            mapping.append((line[0], line[1]))

    return mapping


# Please note that map_cache is persistent between calls and should not be given as argument.
def map_app(appstring, mapping, default='UNKNOWN', map_cache={}):
    '''
    Map the `appstring` on the given `mapping` (list of tuples). Never define `map_cache`!
    Returns the app or `default` if the appstring could not be identified.
    '''

    try:
        return map_cache[appstring]
    except KeyError:
        pass

    for map_re, app in mapping:
        if re.search(map_re, appstring) is not None:
            return app

    return default


def test_map_app():
    mapping = [
        ('^skypeforlinux$', 'Skype'),
        ('^firefox$', 'Firefox'),
        ('[a-z][A-Z][0-9]', 'MyFancyApp'),
        ('^firefox$', 'NOTFirefox')
        ]

    assert map_app('asf', mapping) == 'UNKNOWN'
    assert map_app('firefox', mapping) == 'Firefox'
    assert map_app('aaaxY9zzz', mapping) == 'MyFancyApp'


def create_report(mapping, snap_dir, start, end):

    # FIXME: This should be split into two functions, one reading the files, the other doing the actual parsing for better testing.

    # Add the local timezone to start and end
    utc_offset_sec = time.altzone if time.localtime().tm_isdst else time.timezone
    utc_offset = datetime.timedelta(seconds=-utc_offset_sec)
    start = start.replace(tzinfo=datetime.timezone(offset=utc_offset))
    end = end.replace(tzinfo=datetime.timezone(offset=utc_offset))

    report = defaultdict(float)

    for filename in glob(os.path.normpath(os.path.join(snap_dir, '*'))):
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
                app = map_app(line[4], mapping)
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


@click.command()
@click.option('--output-file', help='Output file. Leave empty or provide - for stdout (default: -).')
@click.option('--map_file', help='Path to the file with the mapping information. If empty, no mapping will be done.')
@click.option('--snap_dir', required=True, help='Path to the directory with the results of sonar snap.')
def do_mapping(output_file,
                  map_file,
                  snap_dir):
    '''
    Map sonar snap results to a provided list of programs and create an output that is suitable for the dashboard etc.
    '''

    if map_file:
        mapping = read_mapping(map_file)
    else:
        mapping = {}

    #today = datetime.date.today()
    today = datetime.datetime.now()
    yesterday = today - datetime.timedelta(days=1)

    report = create_report(mapping, snap_dir, start=yesterday, end=today)

    with write_open(output_file) as f:
        f_writer = csv.writer(f, delimiter='\t', quotechar='"', quoting=csv.QUOTE_MINIMAL)
        for key in sorted(report, key=lambda x: report[x], reverse=True):
            user, project, app = key
            cpu = report[key]
            f_writer.writerow([user, project, app, cpu])


if __name__ == '__main__':
    do_mapping()
