# Guide

The cargo-dist Guide is the "beginner" documentation that walks you through simple usage and then introduces more complex situations as you go. More advanced documentation can be found in "[concepts][]".

If you have a [Cargo Workspace][workspace] with a single Cargo package that `cargo install` works for, and just want zips containing prebuilt binaries for the major desktop platforms, that should Just Work as described in the [Way-Too-Quickstart][way-too-quickstart]. Things get more complicated if you want to:

* have more packages in your [Cargo Workspace][workspace] (libraries, multiple binaries, ...)
* have additional steps in your build (configure the system, add files, ...)
* build various kinds of [installers][] (curl-sh scripts, npm packages, msi, ...)

Gonna be blunt and say that cargo-dist is still in early days and we still need to implement a lot of stuff to better support all the things people want to do with Shippable Builds. If what you want to do doesn't seem properly supported and we don't have [an issue][issues] for it, absolutely file one so we can hash it out!

The guide will start by explaining the simple case, and then explain the more complicated cases.

[concepts]: ./concepts.md
[way-too-quickstart]: ./way-too-quickstart.md
[issues]: https://github.com/axodotdev/cargo-dist/issues
[workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html
[installers]: ./artifacts.md#installers
