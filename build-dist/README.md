# Build Sonar RPMs for HPC distribution

The configuration assets for Sonar are bundled in the RPM, so there will typically be one RPM for
the compute nodes and one for the master node.

The .spec files here build from source.  You normally need to build on the cluster that will take
the RPM, or on a machine with a compatible OS, to get library versions right.

The build scripts in the .spec files will typically need to be customized for your system, minimally
to specify how to initialize dependencies (Rust, C, Binutils, GPU headers).

## RPM cookbook with Fake GPU (Fedora workstation)

For the sake of simplicity:
```
$ sudo dnf install rpmdevtools rpmlint
$ rpmdev-setuptree
```

This creates `~/rpmbuild`, which will hold the RPM assets.  Then:

```
$ cp sonar-node-fakegpu.spec ~/rpmbuild/SPECS
$ ( cd ~/rpmbuild/SOURCES ; wget https://github.com/NordicHPC/sonar/archive/refs/tags/v0.99.1.tar.gz )
```

Obviously the version number matters.  It should be adjusted above, and also in the copied .spec
file.

The .spec depends on an available C compiler and a Rust compiler.  See that file for instructions.

Now to build:
```
$ rpmbuild -bb ~/rpmbuild/SPECS/sonar-node-fakegpu.spec
```

## RPM cookbook with real GPU on HPC node with rpmbuild only

Run this to create the rpm tree:
```
$ ./setuptree.bash
```

Once the .spec has been copied (ideally under a name reflecting the GPU chosen) and sources
downloaded and the version updated in the .spec, it will be necessary to adjust the build tools and
choosing a GPU.  One can use the diff between sonar-node-fakegpu.spec and sonar-node-nvidia.spec
(both in this directory) to guide the process.  Fixing the build tools may involve loading modules
or otherwise installing something.  Choosing the GPU will involve changing the name of the library
that is built and linked and changing the feature set requested from the rust build to correspond to
that GPU.

After that we're back to the build (for the gpu `somegpu`):
```
$ rpmbuild -bb ~/rpmbuild/SPECS/sonar-node-somegpu.spec
```

(Something more here about assets that need to go into the build.)
