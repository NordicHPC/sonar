# Versioning

## Version numbers

The following basic versioning rules are new with v0.8.0.

We use semantic versioning.  The major version is expected to remain at zero for the foreseeable
future, reflecting the experimental nature of Sonar.

The minor version is updated with changes that alter the output format deliberately: fields are
added, removed, or are given a new meaning (this has been avoided so far), or the record format
itself changes.  For example, v0.8.0 both added fields and stopped printing fields that are zero.

The bugfix version is updated for changes that do not alter the output format per se but that might
affect the output nevertheless, ie, most changes not covered by changes to the minor version number.


## Release branches, uplifts and backports

The following branching scheme is new with v0.12.x.

The `main` branch is used for development and has a version number of the form `M.N.O-PRE` where
"PRE" is some arbitrary string, eg "devel", "rc4".  Note that this version number form will also be
present in the output of `sonar ps`, to properly tag those data.  If clients are exposed to
prerelease `ps` data they must be prepared to deal with this.

For every freeze of the the minor release number, a new release branch is created in the repo with
the name `release_<major>_<minor>`, again we expect `<major>` to remain `0` for the foreseeable
future, ergo, `release_0_12` is the v0.12.x release branch.  At branching time, the minor release
number is incremented on main (so when we created `release_0_12` for v0.12.1, the version number on
`main` went to `0.13.0-devel`).  The version number on a release branch is strictly of the form
M.N.O.

When a release `M.N.O` is to be made from a release branch, a tag is created of the form
`release_M_N_O` on that branch and the release is built from that changeset.  Once the release has
shipped, the bugfix version number on the branch is incremented.

With the branches come some additional rules for how to move patches around:

- If a bugfix is made to any release branch and the bug is present on main then the PR shall be
  tagged "uplift-required"; the PR shall subsequently be uplifted main; and following uplift the tag
  shall be changed to "uplifted-to-main".
- If a bugfix is made to main it shall be considered whether it should be backported the most recent
  release branch.  If so, the PR shall be tagged "backport-required"; the PR shall subsequently be
  cherry-picked or backported to the release branch; and following backport the tag shall be changed
  to "backported-to-release".  No older release branches shall automatically be considered for
  backports.


## Policies for changing Rust edition and minimum Rust version

Policy for changing the minimum Rust version:
- Open a GitHub issue and motivate the change
- Once we reach agreement in the issue discussion:
  - Update the version inside the test workflow [test-minimal.yml](.github/workflows/test-minimal.yml)
  - Update the documentation (this section)


## Release pre-testing

Not all testing is automated.  In addition to automated tests, these tests need to be run before
release:

- on a node with no GPUs, run `make test` (this usually happens on github, as the github
  runners have no GPUs)
- on a node with NVIDIA GPUs, run `make test`
- on a node with AMD GPUs, run `make test`
- on a node with a local Kafka install, go to `tests/kafka` and run the tests as described
  in `tests/kafka/README.md`

All of that assumes that the GPU API C code has been properly rebuilt into platform-dependent
libraries after updates.  There are no direct ways of testing that at the moment.  It amounts to
checking that `gpuapi/*/*.a` were updated in git at least as recently as `gpuapi/*.{c,h}`.

There's a little more information in [tests/README.md](../tests/README.md).
