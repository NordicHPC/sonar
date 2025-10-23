# Testing

## General

Testing is divided between white box automated tests embedded in the code, the black box automated
tests in this directory, and some interactive/manual tests in some subdirectories of this directory.

To run the white box automated tests, run `cargo test` in this directory's parent directory.  To run
the black box automated tests, run `run_tests.sh` in this directory.  Both will be run if you run
`make test` in the parent directory.

To run the interactive tests, cd to the various subdirectories of this directory and inspect the
README.md files for instructions.  (Also see below.)

The github workflow runs the automated test suites (and some other things), but the interactive
tests are only run manually.

## Additional release testing

The black box tests in this directory *also need to be run manually* before release *on a variety of
systems* because they sometimes test system-specific aspects.  Assuming that the github runner has
no GPUs and no Slurm available and will test the no-GPU and non-Slurm paths of the code, we
additionally need to run on these types of nodes:

- a node with an NVIDIA GPU (UiO ml[1-3,5-9].hpc nodes, or Fox, Saga, or Betzy GPU nodes)
- a node with an AMD GPU (UiO ml4.hpc node, or Lumi)
- a node with an XPU GPU (Simula n022)
- a node with a Habana GPU (Simula h001)
- a node with Slurm (Fox, Saga, Fram or Betzy login nodes would do)

The tests will probe for GPU and Slurm and enable/disable themselves as appropriate, no
configuration is needed.

## Interactive tests

The interactive tests are:

- `kafka-interactive` tests Sonar's output-to-Kafka-broker functionality with a live broker
- `threads-interactive` tests the ability to record thread counts (this could be made automated)

## Coding standards

All tests should start by sourcing `sh-helper`.  This will create `tmp/` if necessary and set `-e`
and `-o pipefail`.

When `-e` gets in the way, typically around a `grep` that may find no lines, disable as locally as
possible using `set +e` and `set -e`.

Commands that may fail in a way that should cause the test to fail must not be embedded in some
context that will absorb the failure, but must be lifted to the top level and emit output to files
or variables, which can then be tested subsequently.  For example, `test`, `[`, `[[`, and `((` will
consume the error exits of subcommands silently, even in the face of `-e`, as will `for` and
`while`; there are many others, see the manual.  It is hard to write tests that "error out"
properly.  All interesting computation that can fail must happen at the statement level.

Tests that generate temp outputs should place files in `tmp/`.

Tests that use auxiliary input files should name the files similarly to the test (so
`daemon-kafka.ini` goes with `daemon-kafka.sh`).

It's useful for tests to have names that start with the major function tested, when possible (so
`ps-cpu-util.sh` and not just `cpu-util.sh`).

It's useful for every test to print ` Ok` (indented) or some other got-to-the-end phrase (also
indented) at the end of the script.
