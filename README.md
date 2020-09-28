java-preloader-reloadeder
=========================
[![Crates.io](https://img.shields.io/crates/v/jpre?style=flat-square)](https://crates.io/crates/jpre)

Replacement for my old [gist](https://gist.github.com/octylFractal/d85e0b160d8be75dbca29910a2b34f34).

Downloads JDKs from AdoptOpenJDK into `$XDG_CONFIG_HOME/jpre/jdks`, then sets a symlink to the currently active JDK.
The symlinks are per-TTY (which usually implies per-shell), and stored in the Rust-std-determined temporary folder,
which is usually `$TMPDIR` or `/tmp`.

# Installation
Run `cargo install jpre` to get the `jpre` binary, and set your `JAVA_HOME` to `$(jpre java-home)`.
For most people adding this is enough:
```sh
export JAVA_HOME="$(jpre java-home)"
```
This path will be symlinked to the currently selected JDK.
