# Testing

Testing is divided between white box tests embedded in the code and the black box tests in this
directory.  To run the white box tests, run `cargo test` in the parent directory.  To run the black
box tests, run `./run_tests.sh` in this directory.

The github workflow runs both test suites (and some other things).

The black box tests in this directory also need to be run manually before release on a variety of
systems because they sometimes test system-specific aspects.  Assuming that the github runner has no
GPUs and no Slurm available and will test the no-GPU and non-Slurm paths of the code, we
additionally need to run on these types of nodes:

- a node with an NVIDIA GPU (UiO ml[1-3,5-9].hpc nodes, or Fox, Saga, or Betzy GPU nodes)
- a node with an AMD GPU (UiO ml4.hpc node, or Lumi)
- a node with Slurm (Fox, Saga or Betzy would do)

The tests will probe for GPU and Slurm and enable/disable themselves as appropriate, no
configuration is needed.
