# Usage: python3 citation-version.py toml-file cff-file
#
# Given a Cargo.toml and a CITATION.cff, reads the version from the toml and updates the version
# field in the cff.  Skips the update for prerelease versions (those containing '-').
# The cff file is replaced with the rewritten version if everything's successful.

import re
import sys
import tempfile
from pathlib import Path


def read_version(toml_path):
    version_re = re.compile(r'^version\s*=\s*"([^"]*)"')
    for line in Path(toml_path).open():
        if m := version_re.match(line):
            return m.group(1)
    print(f"No version setting found in {toml_path}", file=sys.stderr)
    sys.exit(1)


def update_cff(cff_path, version):
    version_re = re.compile(r"^(version:\s*')[^']*('.*)")
    if "-" in version:
        print(
            f"Can't encode prerelease {version}, skipping translation", file=sys.stderr
        )
        return
    cff = Path(cff_path)
    lines = cff.read_text().splitlines(keepends=True)
    fd, tmp_path = tempfile.mkstemp(dir=cff.parent)
    try:
        with open(fd, "w") as tmp:
            for line in lines:
                if m := version_re.match(line.rstrip("\n")):
                    tmp.write(m.group(1) + version + m.group(2) + "\n")
                else:
                    tmp.write(line)
        Path(tmp_path).replace(cff)
    except Exception as e:
        Path(tmp_path).unlink(missing_ok=True)
        print(f"Error writing {cff_path}: {e}", file=sys.stderr)
        sys.exit(1)


if len(sys.argv) != 3:
    print(f"Usage: {sys.argv[0]} toml-file cff-file", file=sys.stderr)
    sys.exit(2)

update_cff(sys.argv[2], read_version(sys.argv[1]))
