java-preloader-reloadeder
=========================
Replacement for my old [gist](https://gist.github.com/octylFractal/d85e0b160d8be75dbca29910a2b34f34).

Downloads JDKs from AdoptOpenJDK into `$XDG_CONFIG_HOME/jpre/jdks`, then
makes them available to the shell via a wrapper script.

# Installation
Run `cargo install jpre`, and put `wrapper/_java-preloader.sh` wherever you would like, and `source` it
into your shell. This will give you a `jpre` command fully integrated into your shell, allowing
`jpre use` to actually change the `JAVA_HOME` variable in your shell.
 
Note: If the shell integration isn't working, you'll get a dump of the shell code to execute instead.
