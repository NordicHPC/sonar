import sys
import argparse
import configparser
from sonar.snap import take_snapshot
from sonar.map import do_mapping


def main():

    # Inspired by https://stackoverflow.com/q/3609852
    parser = argparse.ArgumentParser(prog='sonar',
                                     description='Tool to profile usage of HPC resources by regularly probing processes.',
                                     epilog='Run sonar <subcommand> -h to get more information about subcommands.')

    subparsers = parser.add_subparsers(title='Subcommands', metavar='', dest='command')

    # create the parser for the "snap" command
    parser_snap = subparsers.add_parser('snap', help='Take a snapshot of the system. Run this on every node and often (e.g. every 15 minutes).')
    parser_snap.add_argument('--output-file', metavar='FILE', help='Output file. Provide - for stdout.')
    parser_snap.add_argument('--cpu-cutoff', metavar='FLOAT', type=float, default=0.5, help='CPU Memory consumption percentage cutoff (default: 0.5).')
    parser_snap.add_argument('--mem-cutoff', metavar='FLOAT', type=float, default=0.0, help='Memory consumption percentage cutoff (default: 0.0).')
    parser_snap.add_argument('--ignored-users', default='avahi, colord, dbus, haveged, polkitd, root, rtkit', help='Users to ignore.')
    parser_snap.add_argument('--hostname-remove', default='.local', help='Hostnames to remove.')
    parser_snap.add_argument('--snap-suffix', default='.tsv', help='Snap file suffix.')
    parser_snap.add_argument('--snap-delimiter', default='\t', help='Snap delimiter.')
    parser_snap.set_defaults(func=take_snapshot)

    # create the parser for the "map" command
    parser_map = subparsers.add_parser('map', help='Parse the system snapshots and map applications. Run this only once centrally and typically once a day.')
    parser_map.add_argument('--snap-dir', metavar='DIR', help='Path to the directory with the results of sonar snap. If empty, the current directory will be assumed.')
    parser_map.add_argument('--output-file', metavar='FILE', help='Output file. Provide - for stdout.')
    parser_map.add_argument('--str-map-file', metavar='FILE', help='Path to the file with the string mapping information.')
    parser_map.add_argument('--re-map-file', metavar='FILE', help='Path to the file with the regexp mapping information.')
    parser_map.add_argument('--default-category', metavar='STR', help='Default category for programs that are not recognized.')
    parser_map.add_argument('--snap-suffix', default='.tsv', help='Snap file suffix.')
    parser_map.add_argument('--snap-delimiter', default='\t', help='Snap delimiter.')
    parser_map.add_argument('--map-suffix', default='.tsv', help='Map file suffix.')
    parser_map.add_argument('--map-delimiter', default='\t', help='Map delimiter.')
    parser_map.set_defaults(func=do_mapping)

    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        sys.exit()

    try:
        # vars() converts object into a dictionary
        args = vars(args)
        args['func'](args)
    except AttributeError:
        parser.print_help()
