

## Installation

Note: I'm using older versions of dependencies (which I generally do
so that the code compiles with rustc from Debian stable, and to reduce
the risk from developer or server system breaches). The code
indirectly depends on ring 0.16.20, which looks like it might use
precompiled binaries, so I'm using `cargo vendor` and rebuild the
binaries in `ring` from source locally before building the
website. You could also `cargo update` instead and adapt the code if
necessary, since recent `ring` explicitly only uses precompiled
binaries on Windows. Or you could trust the systems where those
binaries were built, of course.



IS_DEV=1

