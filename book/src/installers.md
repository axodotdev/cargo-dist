# Installers

<!-- toc -->

The core functionality of cargo-dist is to build your binaries and produce tarballs/zips containing them. Basically every other kind of file it produces is considered an "installer" that helps download/install those binaries.

> Currently all supported installers are "global"/"universal" installers that detect the current platform and download and unpack the appropriate tarball/zip. This currently requires CI support to be enabled so we can ensure the files are hosted somewhere and now where to fetch them from.
>
> In the future we will allow you to specify the download URL manually, and will enable more self-contained "vendored" installers like [Windows .msi][msi-installer-issue] and [macOS .dmg/.app][dmg-installer-issue], as well as [various][linux-pm-issue] [package-managers][windows-pm-issue].


## Supported Installers

Currently supported installers include:

* "shell": a shell script that fetches and installs executables
* "powershell": a powershell script that fetches and installs executables
* "npm": an npm project that fetches and runs executables (e.g. via npx)

These keys can be specified via [`installer` in your cargo-dist config][installer-config]. The [`cargo dist init` command][init] provides an interactive UI for enabling/disabling them.




### shell

> since 0.0.3

This provides a shell script (installer.sh) which detects the current platform, fetches the best possible [executable-zip][] from your [artifact download URL][artifact-download-url], copies the binary into your [install-path][], and attempts to add that path to the user's PATH (see the next section for details).

This kind of installer is ideal for bootstrapping setup on a fairly bare-bones system.

An "installer hint" will be provided that shows how to install via `curl | sh`, like so:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/axodotdev/cargo-dist/releases/download/v0.0.5/cargo-dist-v0.0.5-installer.sh | sh
```

Limitations/Caveats:

* Requires a well-defined [artifact download URL][artifact-download-url]
* Currently only really designed for "linux" and "macOS", and won't detect other platforms properly (and certainly won't play nice with things like nixOS).
* [Cannot detect situations where musl-based builds are appropriate][musl] (static or dynamic) 
* [Relies on the user's installation of `tar` and `unzip` to unpack the files][unpacking]
* Relies on the the user's installation of `curl` or `wget` to fetch the files
* [Will throw out all files except for the binary, so the binary can't rely on assets included in the archive][issue-unpack-all]
* Cannot run any kind of custom install logic

In an ideal world all of these caveats improve (except for maybe relying on tar/unzip/curl/wget, that's kinda fundamental).


#### shell: adding things to PATH

Here is a more fleshed out description of how the shell installer attempts to add the [install-path][] to the user's PATH, and the limitations of that process.

The most fundamental limitation is that installers fundamentally cannot edit the PATH of the currently running shell (it's a parent process). Only an explicit `source some_file` (or the more portable `. some_file`) can do that. As such, it benefits an installer to try to install to a directory that will already be on PATH (such as [CARGO_HOME][cargo home]). Otherwise all we can do is prompt the user to run `source` themselves after the installer has run (or restart their shell to freshly source rcfiles).

The process we use to add [install-path][] to the user's PATH is roughly the same process that rustup uses (hopefully making us harmonious with running rustup before/after one of our installer scripts). In the following description we will use `$install-path` as a placeholder for the path computed at install-time where the binaries get installed. Its actual value will likely look something like `$HOME/.myapp` or `$HOME/.cargo/bin`.

* we generate a shell script and write it to `$install-path/env` (let's call this `$env-path`)
    * the script checks if `$install-path` is in PATH already, and prepends it if not
    * prepending is used to ideally override system-installed binaries, as that is assumed to be desired when explicitly installing with not-your-system-package-manager
    * the `env` script will only be added if it doesn't already exist
    * if `install-path = "CARGO_HOME"`, then `$env-path` will actually be in the parent directory, mirroring the behaviour of rustup
* we add `. $env-path` to `$HOME/.profile`
    * this is just a more portable version of `source $install-path/env`
    * this line will only be added if it doesn't exist (we also check for the `source` equivalent)
    * the file is created if it doesn't exist
    * [rustup shotgun blasts this line into many more files like .bashrc and .zshenv](https://github.com/rust-lang/rustup/blob/bcfac6278c7c2f16a41294f7533aeee2f7f88d07/src/cli/self_update/shell.rs#L70-L76), while still [lacking proper support for fish](https://github.com/rust-lang/rustup/issues/478) and other more obscure shells -- we opted to start conservative with just .profile
* if `$HOME/.profile` was edited, we prompt the user to `source "$env-path"` or restart their shell
    * although this is less portable than `. "$env-path"`, it's very easy to misread/miscopy the portable version (not as much of a concern for an rcfile, but an issue for humans)
    * hopefully folks on platforms where this matters are aware of this issue (or they can restart their shell)



### powershell

> since 0.0.3

This provides a powershell script (installer.ps1) which detects the current platform, fetches the best possible [executable-zip][] from your [artifact download URL][artifact-download-url], copies the binary into your [install-path][], and attempts to add that path to the user's PATH (see the next section for details).

This kind of installer is ideal for bootstrapping setup on a fairly bare-bones system.

An "installer hint" will be provided that shows how to install via `irm | iex` (the windows equivalent of `curl | sh`), like so:

```sh
irm https://github.com/axodotdev/cargo-dist/releases/download/v0.0.5/cargo-dist-v0.0.5-installer.ps1 | iex
```

Limitations/Caveats:

* Requires a well-defined [artifact download URL][artifact-download-url]
* Currently only really designed for "native windows", and won't detect other platforms properly
* [Cannot detect situations where musl-based builds are appropriate][musl] (static or dynamic) 
* [Relies on the user's installation of `tar` and `Expand-Archive` to unpack the files][unpacking]
* Relies on the the user's installation of `Net.Webclient` to fetch the files
* [Won't work if run in cmd instead of powershell][issue-irm-iex]
* [Will throw out all files except for the binary, so the binary can't rely on assets included in the archive][issue-unpack-all]
* Cannot run any kind of custom install logic

On the scale of Windows (where many people are still running Windows 7) commands like "Expand-Archive" and "tar" are in fact relatively new innovations. Any system that predates 2016 (PowerShell 5.0) certainly has no hope of working. I believe that someone running Windows 10 is basically guaranteed to work, and anything before that gets sketchier.

In an ideal world most of these caveats improve (except for maybe the requirement of PowerShell >= 5.0 which is not pleasant to push past).


#### powershell: adding things to PATH

Here is a more fleshed out description of how the powershell installer attempts to add the [install-path][] to the user's PATH, and the limitations of that process.

The most fundamental limitation is that installers fundamentally cannot edit the PATH of the currently running shell (it's a parent process). Powershell does not have an equivalent of `source`, so to the best of our knowledge restarting the shell is the only option (which if using Windows Terminal seems to mean opening a whole new window, tabs aren't good enough). As such, it benefits an installer to try to install to a directory that will already be on PATH (such as [CARGO_HOME][cargo home]). ([rustup also sends a broadcast WM_SETTINGCHANGE message](https://github.com/rust-lang/rustup/blob/bcfac6278c7c2f16a41294f7533aeee2f7f88d07/src/cli/self_update/windows.rs#L397-L409), but we couldn't find any evidence that this does anything useful.)

The process we use to add [install-path][] to the user's PATH is roughly the same process that rustup uses (hopefully making us harmonious with running rustup before/after one of our installer scripts). In the following description we will use `$install-path` as a placeholder for the path computed at install-time where the binaries get installed. Its actual value will likely look something like `C:\Users\axo\.myapp` or `C:\Users\.cargo\bin`.

* we load from the registry `HKCU:\Environment`'s "Path" Item
* we check if `$install-path` is contained within it already
* if not, we prepend it and write the value back
    * prepending is used to ideally override system-installed binaries, as that is assumed to be desired when explicitly installing with not-your-system-package-manager
* if we edited the registry, we prompt the user to restart their shell





### npm

> since 0.0.6

This provides a tarball containing an npm package (npm-package.tar.gz) which when installed into an npm project: detects the current platform, fetches the best possible [executable-zip][] from your [artifact download URL][artifact-download-url], and copies the binary into your node_modules. This can be used to install the binaries like any other npm package, or to run them with `npx`.

This kind of installer is ideal for making a native Rust tool available to JS devs.

An "installer hint" will be provided that shows how to install via `npm` like so:

```sh
npm install @axodotdev/cargodisttest@0.2.0
```

**cargo-dist does not publish the package for you, you need to do that manually once the tarball is built.** Conveniently, npm supports publishing from a url-to-a-tarball directly, and since 0.0.7 we make our tarballs look like "proper" npm package tarballs, so you can just do this:

```sh
npm publish URL_TO_TARBALL
```

You can find the URL to the tarball at the bottom of the Github Release, inside the collapsible "assets" dropdown (*-npm-package.tar.gz). The format of the url is:

```text
<repo>/releases/download/<tag>/<app-name>-npm-package.tar.gz
```

Example:

https://github.com/axodotdev/oranda/releases/download/v0.0.3/oranda-npm-package.tar.gz

If you're cutting a stable release (not-prerelease), you can use the "latest" URL format:

https://github.com/axodotdev/oranda/releases/latest/download/oranda-npm-package.tar.gz

In the future we may [introduce more streamlined CI-based publishing workflows][issue-npm-ci].

[You can set the @scope the package is published under with the npm-scope cargo-dist config][npm-scope].

We will otherwise do our best to faithfully translate [any standard Cargo.toml values you set][cargo-manifest] to an equivalent in the npm package.json format (name, version, authors, description, homepage, repository, keywords, categories...).

The package will also include an npm-shrinkwrap.json file for the npm packages the installer uses, this is the same as package-lock.json but "really for reals I want this to be respected even if it's installed into another project". Note that [cargo install similarly disrespects Cargo.lock unless you pass --locked][install-locked].




Limitations/Caveats:

* Requires a well-defined [artifact download URL][artifact-download-url]
* [Cannot detect situations where musl-based builds are appropriate][musl] (static or dynamic) 
* [Relies on nodejs's builtin gzip support to unpack the files, which only works with .tar.gz][unpacking]
* Cannot run any kind of custom install logic

As a result of the `.tar.gz` limitation, `cargo dist init` will prompt you to change [windows-archive][] and [unix-archive][] to ".tar.gz" if you enable the npm installer, erroring if you decline.




## Artifact Download URL

All installers which rely on detecting the current platform and fetching "your" [executable-zips][] (archives) to install prebuilt binaries need to know where to fetch from. They do this by combining a base URL with the precomputed name of the archive. That base URL is the *Artifact Download URL*.

The Artifact Download URL effectively mandates that all archives for a Release must be stored in the same directory (or pretend to be with redirects), and must have the exact name that cargo-dist selected for them.

The Artifact Download URL is currently on defined if:

* [You have enabled Github CI][github-ci]
* [All crates in your workspace agree on the Cargo "repository" key][repository-url]

"Agree" here means that:

* At least one crate defines the key
* Every other crate that bothers to set the key has the same value (modulo trailing "/")

If this is the case, then it will be:

```text
{{repository}}/releases/download/{{tag}}
```

For instance the Artifact Download URL for cargo-dist 0.0.5 is:

```text
https://github.com/axodotdev/cargo-dist/releases/download/v0.0.5/
```

In the future this will be made more configurable.




## Unpacking Files

cargo-dist theoretically allows you to build [executable-zips][] with any of the following formats:

* .tar.gz
* .tar.xz
* .tar.zstd
* .zip

(See [windows-archive][] and [unix-archive][] for details and defaults)

But that doesn't necessarily mean a random user can unpack those formats, and that *especially* doesn't mean an installer that's trying to bootstrap the installation by fetching one of those archives can. This section serves to document some known limitations of various systems' builtin unpacking utilities.

* On unix-y platforms `tar` tends to be available with .tar.gz and .tar.xz well-supported, but not .tar.zstd. `unzip` is also pretty standard for handling .zip files.

* Modern Windows (~Windows 10) has a copy of bsd `tar`, but it *only* supports .tar.gz out of the box (and zip I think, but we use the similarly-new Expand-Archive command for that). The windows file explorer also seemingly has no idea how to open a .tar.gz, unlike a .zip which just pops open with a double click, so worse UX for anyone manually falling back to the raw archives. Both of these are relatively new commands that older Windows systems might lack (introduced in ~2016/2017).

* The npm `binary-install` and `tar` packages only support .tar.gz (because nodejs provides a builtin gzip decoder and they just rely on that). There are seemingly packages for other formats but we have yet to cobble together a comprehensive implementation that combines them all.

* The Rust ecosystem similarly requires individual packages for all these formats, but they all have pretty simple/uniform APIs so we were able to cobble together basic support without too much effort.





## Other Installation Methods

cargo-dist projects can also theoretically be installed with the following, through no active effort of our own:

* [cargo-install][] (just [cargo publish][] like normal)
* [cargo-binstall][] (the URL schema we use for Github Releases is auto-detected)

In the future we might [support displaying these kinds of install methods][issue-info-install].

Note that cargo-install is just building from the uploaded source with the --release profile, and so if you're relying on cargo-dist or unpublished files for some key behaviours, this may cause problems. [It also disrespects your lockfile unless you pass --locked][install-locked]. You can more closely approximate cargo-dist's build with:

```sh
cargo install --profile=dist --locked
```

Although that's still missing things like [Windows crt-static workarounds][crt-static].




[issue-info-install]: https://github.com/axodotdev/cargo-dist/issues/72
[issue-npm-ci]: https://github.com/axodotdev/cargo-dist/issues/245
[linux-pm-issue]: https://github.com/axodotdev/cargo-dist/issues/76
[windows-pm-issue]: https://github.com/axodotdev/cargo-dist/issues/87
[msi-installer-issue]: https://github.com/axodotdev/cargo-dist/issues/23
[dmg-installer-issue]: https://github.com/axodotdev/cargo-dist/issues/24
[issue-unpack-all]: https://github.com/axodotdev/cargo-dist/issues/307
[issue-irm-iex]: https://github.com/axodotdev/oranda/issues/393
[installer-config]: ./config.md#installers
[executable-zip]: ./artifacts.md#executable-zip
[executable-zips]: ./artifacts.md#executable-zip
[init]: ./cli.md#cargo-dist-init
[artifact-download-url]: #artifact-download-url
[cargo home]: https://doc.rust-lang.org/cargo/guide/cargo-home.html
[cargo-binstall]: https://github.com/cargo-bins/cargo-binstall
[cargo-install]: https://doc.rust-lang.org/cargo/commands/cargo-install.html
[cargo publish]: https://doc.rust-lang.org/cargo/commands/cargo-publish.html
[unpacking]: #unpacking-files
[npm-targz]: https://github.com/axodotdev/cargo-dist/issues/226
[musl]: https://github.com/axodotdev/cargo-dist/issues/75
[npm-scope]: ./config.md#npm-scope
[unix-archive]: ./config.md#unix-archive
[windows-archive]: ./config.md#windows-archive
[github-ci]: ./config.md#ci
[repository-url]: ./config.md#repository
[install-path]: ./config.md#install-path
[cargo-manifest]: https://doc.rust-lang.org/cargo/reference/manifest.html
[install-locked]: https://doc.rust-lang.org/cargo/commands/cargo-install.html#dealing-with-the-lockfile
[crt-static]: https://github.com/rust-lang/rfcs/blob/master/text/1721-crt-static.md