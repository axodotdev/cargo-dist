# Self-updater

> since 0.12.0

NOTE: This feature is currently experimental.

Ordinarily, your users will need to visit your website and download an installer for the latest release in order to upgrade. Users who installed your software via a package manager, like Homebrew and npm, can use that package manager to upgrade to the latest release. For users of the [shell] and [PowerShell] installers, you can provide your users with a standalone installation program to upgrade more conveniently.

If you add `install-updater = true` to your `Cargo.toml`, cargo-dist's shell and PowerShell installers will include an updater program alongside your program itself. This standalone program will be installed as the name `yourpackage-update`, and users can simply run it to poll for new releases and have them installed. The source code for this program is open source in the [axoupdater] repository.

Users will interact with this updater by running the `yourpackage-update` command. It takes no options or arguments, and will automatically perform an upgrade without further input from the user. If your program supports custom external subcommands via the executable naming structure, like `git` and `cargo` do, then your user can also run `yourpackage update`. Here's a sample `axolotlsay-update` session as a demonstration of what your users will experience:

```
$ axolotlsay-update
Checking for updates...
downloading axolotlsay 0.2.114 aarch64-apple-darwin
installing to /Users/mistydemeo/.cargo/bin
  axolotlsay
  axolotlsay-update
everything's installed!
New release installed!
```

If you would prefer to handle polling for updates yourself, for example in order to incorporate it as an internal subcommand of your own software, axoupdater is available as a [crate] which can be used as a library within your program. More information about how to use axoupdater as a library in your own program can be found in its README and in its [API documentation][axoupdater-docs].

## GitHub Actions and Rate Limits in CI

By default, axoupdater uses unauthenticated GitHub API calls when fetching release information. This is reliable in normal use, but it's much more likely to run into rate limits in the highly artificial environment of a CI test. If you're testing the standalone updater in your CI configuration, we recommend setting the `AXOUPDATER_GITHUB_TOKEN` environment variable to the value of the `GITHUB_TOKEN` secret that GitHub Action defines automatically.

```yaml
env:
  AXOUPDATER_GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

A sample in cargo-dist's CI configuration can be found [here][cargo-dist-ci-config].

If you use the axoupdater crate to implement the updater yourself, instructions for opting into a token in CI can be found [here][axoupdater-token-docs].

[axoupdater]: https://github.com/axodotdev/axoupdater
[axoupdater-docs]: https://docs.rs/axoupdater/
[axoupdater-token-docs]: https://github.com/axodotdev/axoupdater?tab=readme-ov-file#github-actions-and-rate-limits-in-ci
[cargo-dist-ci-config]: https://github.com/axodotdev/cargo-dist/blob/80f2e19e5aa79b7b1f64beb62ceb07aa71566707/.github/workflows/ci.yml#L82-L85
[crate]: https://crates.io/crates/axoupdater
[shell]: ../shell.md
[PowerShell]: ../powershell.md
