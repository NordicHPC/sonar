# Installing Sonar

To build, you need to have gcc11 or newer, rust/cargo 1.81 or newer, and openssl header files.  See
`doc/HOWTO-DEVELOP.md` if in doubt.

To build Sonar on most recent x86_64 or aarch64 Linux systems with support for a range of GPUs:

```
$ make
```

This creates `target/release/sonar` which can be copied freely to other compatible distros.  There
is no default installation method.

See `doc/HOWTO-DEVELOP.md` for more about more complex compilation scenarios.

See `doc/HOWTO-DEPLOY.md` for more about installation.
