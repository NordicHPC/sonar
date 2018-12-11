#!/usr/bin/env python3

import sys
import argparse
import configparser


def sonar_snap(args):
    from sonar.snap import take_snapshot

    take_snapshot(args)


def sonar_map(args):
    from sonar.map import do_mapping

    do_mapping(args)


def main():
    defaults = {'snap_suffix': '.tsv',
                'map_suffix': '.tsv',
                'snap_delimiter': '\t',
                'map_delimiter': '\t',
                'cpu_cutoff': 0.5,
                'mem_cutoff': 0.0,
                'hostname_remove': '',
                'ignored_users': '',
                'snap_dir': '.',
                'str_map_file': '',
                're_map_file': '',
                'default_category': 'UNKNOWN',
                'output_file': None}

    # Inspired by https://stackoverflow.com/q/3609852
    parser = argparse.ArgumentParser(prog='sonar', description='Tool to profile usage of HPC resources by regularly probing processes.', epilog='Run sonar <subcommand> -h to get more information about subcommands.')

    subparsers = parser.add_subparsers(title='Subcommands', metavar='', dest='command')

    # create the parser for the "snap" command
    parser_snap = subparsers.add_parser('snap', help='Take a snapshot of the system. Run this on every node! Supposed to run often (e.g. every 15 minutes).')
    parser_snap.add_argument('--config', default='example_config.conf', help='Path to config file')
    parser_snap.add_argument('--output-file', metavar='FILE', help='Output file. Provide - for stdout.')
    parser_snap.add_argument('--cpu-cutoff', metavar='FLOAT', type=float, help='CPU Memory consumption percentage cutoff (default: 0.5).')
    parser_snap.add_argument('--mem-cutoff', metavar='FLOAT', type=float, help='Memory consumption percentage cutoff (default: 0.0).')
    parser_snap.set_defaults(func=sonar_snap)

    # create the parser for the "map" command
    parser_map = subparsers.add_parser('map', help='Parse the system snapshots and map applications. Run this only once centrally. Supposed to run e.g. once a day.')
    parser_map.add_argument('--config', default='example_config.conf', help='Path to config file')
    parser_map.add_argument('--snap-dir', metavar='DIR', help='Path to the directory with the results of sonar snap. If empty, the current directory will be assumed.')
    parser_map.add_argument('--output-file', metavar='FILE', help='Output file. Provide - for stdout.')
    parser_map.add_argument('--str-map-file', metavar='FILE', help='Path to the file with the string mapping information.')
    parser_map.add_argument('--re-map-file', metavar='FILE', help='Path to the file with the regexp mapping information.')
    parser_map.add_argument('--default-category', metavar='STR', help='Default category for programs that are not recognized.')
    parser_map.set_defaults(func=sonar_map)
    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        sys.exit()

    if args.config:
        # If a config file was given, parse it and reparse the command line arguments
        config = configparser.ConfigParser()
        try:
            config.read_file(open(args.config))
        except FileNotFoundError:
            print('ERROR: file "{0}" not found'.format(args.config), file=sys.stderr)
            sys.exit()
        defaults.update(dict(config.items(args.command)))

        # Kind of hacky, but floats have to be casted individually
        for key in ['cpu_cutoff', 'mem_cutoff']:
            defaults[key] = float(defaults[key])

        parser_snap.set_defaults(**defaults)
        parser_map.set_defaults(**defaults)

        # Reparse arguments, this time knowing the defaults
        args = vars(parser.parse_args())

    # Manual parsing of "difficult" values
    for key in ['snap_delimiter', 'map_delimiter']:
        if key in args and args[key] in (r'\t', '{tab}', '<tab>'):
            args[key] = '\t'
    args['ignored_users'] = [u.strip() for u in args['ignored_users'].split(',')]

    try:
        args['func'](args)
    except AttributeError:
        parser.print_help()
