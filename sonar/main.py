from .cli import parse_args
from .snapshot import take_snapshot
import sys


def main():
    args = parse_args()
    if args.snapshot:
        if args.filename is None:
            sys.stderr.write('ERROR: --snapshot requires setting --file\n')
            sys.exit(1)
        take_snapshot(args.filename)


def test_something():
    assert 1 == 1
