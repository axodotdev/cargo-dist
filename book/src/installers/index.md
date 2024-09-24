# Installers

The core functionality of cargo-dist is to build your binaries and produce [tarballs / zips][archives] containing them. Basically every other kind of output it produces is considered an "installer" that helps download/install/run those binaries.

Note that we use the term "installer" very loosely -- if it's fancier than a tarball, it's an installer to us!


## Supported Installers

Currently supported installers include:

* [shell][]: a shell script that fetches and installs executables (for `curl | sh`)
* [powershell][]: a powershell script that fetches and installs executables (for `irm | iex`)
* [npm][]: an npm project that fetches and runs executables (for `npx`)
* [homebrew][]: a Homebrew formula that fetches and installs executables
* [msi][]: a Windows msi that bundles and installs executables
* [pkg][]: a Mac pkg that bundles and installs executables

These keys can be specified via [`installer` in your cargo-dist config][config-installers]. The [`cargo dist init` command][init] provides an interactive UI for enabling/disabling them.

The above installers can have one of two strategies: *fetching* and *bundling* (defined below). Currently each installer is hardcoded to one particular strategy, but in the future [we may make it configurable][issue-unlock-installers].


## Future Installers

The following installers have been requested, and we're open to supporting them, but we have no specific timeline for when they will be implemented. Providing additional info/feedback on them helps us prioritize the work:

* [linux docker image containing binaries](https://github.com/axodotdev/cargo-dist/issues/365)
* [linux flatpak](https://github.com/axodotdev/cargo-dist/issues/25)
* [macOS cask](https://github.com/axodotdev/cargo-dist/issues/309)
* [macOS dmg / app](https://github.com/axodotdev/cargo-dist/issues/24)
* [pypi package](https://github.com/axodotdev/cargo-dist/issues/86)
* [windows winget package](https://github.com/axodotdev/cargo-dist/issues/87)



## Fetching Installers

Fetching installers are thin wrappers which detect the user's current platform and download and unpack the appropriate [archive][archives] from a server.

In exchange for requiring [a well-defined Artifact URL][artifact-url] and an internet connection at install-time, this strategy gives you a simple and efficient way to host prebuilt binaries and make sure that all users get the same binaries regardless of how the installed your application.

Fetching installers are also easy to make "universal" (cross-platform), so your installing users don't need to care about the OS or CPU they're using -- the installer will handle that for them.

Installers which support fetching:

* [shell][]: a shell script that fetches and installs executables (for `curl | sh`)
* [powershell][]: a powershell script that fetches and installs executables (for `irm | iex`)
* [npm][]: an npm project that fetches and runs executables (for `npx`)
* [homebrew][]: a Homebrew formula that fetches and installs executables


## Bundling Installers

Bundling installers contain the actual binaries they will install on the user's system.

These installers can work without any internet connection, which some users will demand or appreciate.

Bundling requires a fundamental compromise when it comes to "universal" (cross-platform) installers, as any installer that wants to support e.g. [Intel macOS and Apple Silicon macOS][issue-macos-universal] will need to include both binaries, even if only one will ever be used.

For this reason all bundling installers are currently single-platform, requiring the installing user to know what platform they're on.

Installers which support bundling:

* [msi][]: a Windows msi that bundles and installs executables




[config-installers]: ../reference/config.md#installers

[issue-unlock-installers]: https://github.com/axodotdev/cargo-dist/issues/450
[issue-info-install]: https://github.com/axodotdev/cargo-dist/issues/72
[issue-macos-universal]: https://github.com/axodotdev/cargo-dist/issues/77

[shell]: ./shell.md
[powershell]: ./powershell.md
[msi]: ./msi.md
[npm]: ./npm.md
[homebrew]: ./homebrew.md
[pkg]: ./pkg.md

[archives]: ../artifacts/archives.md
[artifact-url]: ../reference/artifact-url.md
[init]: ../reference/cli.md#cargo-dist-init
