java-preloader-reloadeder
=========================
[![Crates.io](https://img.shields.io/crates/v/jpre?style=flat-square)](https://crates.io/crates/jpre)

Replacement for my old [gist](https://gist.github.com/octylFractal/d85e0b160d8be75dbca29910a2b34f34).

Downloads JDKs from Adoptium into `<cache dir>/jpre/jdks`, then sets a symlink to the currently active JDK.
The symlinks are per-environment (which usually implies per-shell), and stored in the Rust-std-determined temporary
folder, which is usually `$TMPDIR` or `/tmp`. Note that the folder used is determined when initialized, so it won't
change on later invocations using the same initialized environment.

# Installation
Run `cargo install jpre` to get the `jpre` binary, and add this to your shell initialization:
```sh
# Initializes jpre's designated JAVA_HOME, using a random location. This will be symlinked to the currently active JDK.
export JPRE_JAVA_HOME="$(jpre generate-java-home-location)"
# Set the current JAVA_HOME to jpre's.
export JAVA_HOME="$(jpre java-home)"
# Puts the binaries on your path.
export PATH="$JAVA_HOME/bin:$PATH"
# Potentially optional, forces shell to re-scan for `java` et. al.
hash -r
```

Note that if you do not set a default JDK, the symlinked path will lead nowhere!

# Usage
Run `jpre use 11`, this downloads JDK 11 from Adoptium and makes it the active JDK.
Other major versions can be downloaded and configured using `jpre use <major>`.
The default JDK can be set using `jpre default <major>`.

Full details are available by running `jpre help`.
