java-preloader-reloadeder
=========================
[![Crates.io](https://img.shields.io/crates/v/jpre?style=flat-square)](https://crates.io/crates/jpre)

Replacement for my old [gist](https://gist.github.com/octylFractal/d85e0b160d8be75dbca29910a2b34f34).

Downloads JDKs from [foojay DiscoAPI](https://api.foojay.io/swagger-ui/) and stores them in `<cache dir>/jpre/jdks`.
`<cache dir>` is based on [
`directories`](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html#method.cache_dir).

Preferred distribution can be set in the config (also `directories`-based), defaults to `temurin`.

# Installation

Run `cargo install jpre` to get the `jpre` binary.

Then, do the following (or similar) in your shell's startup script:

```sh
# This may be necessary to ensure the context is understood everywhere
export JPRE_CONTEXT_ID="$(jpre get-context-id)"
# Sets the Java home to the jpre-managed location. This will be symlinked to the currently active JDK.
export JAVA_HOME="$(jpre java-home)"
# Puts the binaries on your path
export PATH="$JAVA_HOME/bin:$PATH"
# Potentially optional, forces shell to re-scan for `java` et. al
hash -r
```

Note that if you do not set a default JDK (with `jpre default`), the symlinked path will lead nowhere!

# Usage

Run e.g. `jpre use 17`, this downloads Temurin JDK 17 and makes it the active JDK.
Other major versions can be downloaded and configured using `jpre use <major>`.
The default JDK can be set using `jpre default <major>`.

Full details are available by running `jpre help`.

# How it works

`jpre` uses the parent process ID as a key to determine the symlink location. This makes it work per-shell (or other
process) and not interfere with other sessions. The symlink is in the `jpre` cache directory and is updated every time
a JDK is selected.

In order to prevent cross-session pollution, `java-home` clears any existing symlink before creating a new one.
