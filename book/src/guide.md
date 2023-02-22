# Guide

The cargo-dist Guide is the "beginner" documentation that walks you through simple usage and then introduces more complex situations as you go. More advanced documentation can be found in "[concepts][]".

If you have a [Cargo Workspace][workspace] with a single Cargo package that `cargo install` works for, and just want zips containing prebuilt binaries for the major desktop platforms, that should Just Work as described in the [Way-Too-Quickstart][way-too-quickstart]. Things get more complicated if you want to:

* have more packages in your [Cargo Workspace][workspace] (libraries, multiple binaries, ...)
* have additional steps in your build (configure the system, add files, ...)
* build various kinds of [installers][] (curl-sh scripts, npm packages, msi, ...)

Gonna be blunt and say that cargo-dist is still in early days and we still need to implement a lot of stuff to better support all the things people want to do with Shippable Builds. If what you want to do doesn't seem properly supported and we don't have [an issue][issues] for it, absolutely file one so we can hash it out!

The guide will start by explaining the simple case, and then explain the more complicated cases.




[simple-app-manifest]: ./img/simple-app-manifest.png
[simple-app-manifest-with-files]: ./img/simple-app-manifest-with-files.png

[install]: ./install.md
[concepts]: ./concepts.md
[way-too-quickstart]: ./way-too-quickstart.md
[issues]: https://github.com/axodotdev/cargo-dist/issues
[workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html
[installers]: TODO://link-to-installers-info
[bin]: https://doc.rust-lang.org/cargo/reference/cargo-targets.html#binaries
[config]: ./config.md
[cargo-release]: https://github.com/crate-ci/cargo-release
[git-tag]: https://git-scm.com/book/en/v2/Git-Basics-Tagging
[init]: TODO://link-to-init-command
[generate-ci]: TODO://link-to-generate-ci-command
[cargo-profile]: https://doc.rust-lang.org/cargo/reference/profiles.html
[thin-lto]: https://doc.rust-lang.org/cargo/reference/profiles.html#lto
[tunning]: TODO://link-to-tuning
[workspace-metadata]: https://doc.rust-lang.org/cargo/reference/workspaces.html#the-metadata-table
[rust-version]: https://doc.rust-lang.org/cargo/reference/manifest.html#the-rust-version-field
[rustup]: https://rustup.rs/
[platforms]: https://doc.rust-lang.org/nightly/rustc/platform-support.html
[release-yml]: https://github.com/axodotdev/cargo-dist/blob/main/.github/workflows/release.yml
[jq]: https://stedolan.github.io/jq/
[manifest]: TODO://link-to-manifest-command
[build]: TODO://link-to-build-command
[artifact-modes]: TODO://link-to-artifact-modes