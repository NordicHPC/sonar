# Testing

## General

Testing is divided between white box automated tests embedded in the code, the black box automated
tests in this directory, and some interactive/manual tests in some subdirectories of this directory.

To run the white box automated tests, run `cargo test` in this directory's parent directory.

To run the black box automated tests, run `./run_tests.sh` in this directory.

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
- a node with Slurm (Fox, Saga, Fram or Betzy login nodes would do)

The tests will probe for GPU and Slurm and enable/disable themselves as appropriate, no
configuration is needed.

## Interactive tests

The interactive tests are:

- `directory` tests Sonar's output-to-directory-tree functionality
- `kafka` tests Sonar's output-to-Kafka-broker functionality
