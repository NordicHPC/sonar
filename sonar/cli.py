#!/usr/bin/env python3

import argparse


def sonar_snap(args):
    from sonar.snap import take_snapshot

    take_snapshot(output_file=args.output_file, cpu_cutoff=args.cpu_cutoff, mem_cutoff=args.mem_cutoff)


def sonar_map(args):
    from sonar.map import do_mapping

    do_mapping(output_file=args.output_file, map_file=args.map_file, snap_dir=args.snap_dir)


def main():

    # create the top-level parser
    parser = argparse.ArgumentParser(prog='sonar', description='Tool to profile usage of HPC resources by regularly probing processes.', epilog='Run sonar <subcommand> -h to get more information about subcommands.')
    # Arguments common to *all* subcommands may be added to parser with parser.add_argument(...)
    subparsers = parser.add_subparsers(title='Subcommands', metavar='')

    # create the parser for the "snap" command
    parser_snap = subparsers.add_parser('snap', help='Take a snapshot of the system. Run this on every node! Supposed to run often (e.g. every 15 minutes).')
    parser_snap.add_argument('--output-file', default='', help='Output file. Leave empty or provide - for stdout.')
    parser_snap.add_argument('--cpu-cutoff', type=float, default=0.5, help='CPU Memory consumption percentage cutoff (default: 0.5).')
    parser_snap.add_argument('--mem-cutoff', type=float, default=0.0, help='Memory consumption percentage cutoff (default: 0.0).')
    parser_snap.set_defaults(func=sonar_snap)

    # create the parser for the "map" command
    parser_map = subparsers.add_parser('map', help='Parse the system snapshots and map applications. Run this only once centrally. Supposed to run e.g. once a day.')
    parser_map.add_argument('--snap-dir', default='', help='Path to the directory with the results of sonar snap. If empty, the current directory will be assumed.')
    parser_map.add_argument('--output-file', default='', help='Output file. Leave empty or provide - for stdout.')
    parser_map.add_argument('--map-file', default='', help='Path to the file with the mapping information. If empty, no mapping will be done (all apps will be "UNKNOWN").')
    parser_map.set_defaults(func=sonar_map)

    # parse some argument lists
    args = parser.parse_args()

    try:
        args.func(args)
    except AttributeError:
        parser.print_help()
