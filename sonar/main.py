from .cli import parse_args


def main():
    args = parse_args()

    print('snapshot', args.snapshot)
    print('file name', args.filename)


def test_something():
    assert 1 == 1
