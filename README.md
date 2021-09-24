java-preloader-reloadeder
=========================
[![Crates.io](https://img.shields.io/crates/v/jpre?style=flat-square)](https://crates.io/crates/jpre)

Replacement for my old [gist](https://gist.github.com/octylFractal/d85e0b160d8be75dbca29910a2b34f34).

Downloads JDKs from Adoptium into `<cache dir>/jpre/jdks`, then sets a symlink to the currently active JDK.
The symlinks are per-TTY (which usually implies per-shell), and stored in the Rust-std-determined temporary folder,
which is usually `$TMPDIR` or `/tmp`.

# Installation
Run `cargo install jpre` to get the `jpre` binary, and set your `JAVA_HOME` to `$(jpre java-home)`.

For most people adding this is enough:
```sh
# Retrieves the TTY-specific path and stores it in JAVA_HOME. This will be symlinked to the currently active JDK.
export JAVA_HOME="$(jpre java-home)"
# Puts the binaries on your path
export PATH="$JAVA_HOME/bin:$PATH"
# Potentially optional, forces shell to re-scan for `java` et. al
hash -r
```

Note that if you do not set a default JDK, the symlinked path will lead nowhere!

# Usage
Run `jpre use 11`, this downloads JDK 11 from Adoptium and makes it the active JDK.
Other major versions can be downloaded and configured using `jpre use <major>`.
The default JDK can be set using `jpre default <major>`.

Full details are available by running `jpre help`.

# Known Limitations
Since this is per-TTY, closing and re-opening a terminal tab / window may result in a different JDK than the default, due to TTY reuse.
