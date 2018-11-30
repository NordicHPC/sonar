from argparse import ArgumentParser, ArgumentDefaultsHelpFormatter


def parse_args():
    parser = ArgumentParser(formatter_class=ArgumentDefaultsHelpFormatter)
    arg = parser.add_argument

    arg('--snapshot', action='store_true', help='take a process snapshot')
    arg('--file', '-f', dest='filename', help='output file')

    return parser.parse_args()
