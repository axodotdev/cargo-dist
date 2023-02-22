# Supported Workflows

> It may be helpful to read the "[concepts][]" section but we'll try to summarize things as we go.

If you have a workspace with a single Cargo package that `cargo install` works for and just want zips containing prebuilt binaries for the major desktop platforms, that should Just Work as described in the [Way-Too-Quickstart][way-too-quickstart]. Things get more complicated if you:

* want to have more packages in your workspace (libraries, multiple binaries, ...)
* want to have additional steps in your build (configure the system, add files, ...)
* want to build various kinds of installers (curl-sh scripts, npm packages, msi, ...)

Gonna be blunt and say that cargo-dist is still in early days and we still need to implement a lot of stuff to better support all the things people want to do with Shippable Builds. If what you want to do doesn't seem properly supported and we don't have [an issue][issues] for it, absolutely file one so we can hash it out!

With that established, let's get into how to do more complicated things, starting with workspaces!





[concepts]: ./concepts.html
[way-too-quickstart]: ./way-too-quickstart.md
[issues]: https://github.com/axodotdev/cargo-dist/issues