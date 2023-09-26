# Shell Script Installer

> since 0.0.3

The "shell" installer provides a shell script (my-app-installer.sh) which detects the current platform, fetches the best possible [archive][] from your [Artifact URL][artifact-url], copies the binary into your [install-path][config-install-path], and attempts to add that path to the user's PATH (see the next section for details).

This kind of installer is ideal for bootstrapping setup on a fairly bare-bones system.

An "installer hint" will be provided that shows how to install via `curl | sh`, like so:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/axodotdev/cargo-dist/releases/download/v0.0.5/cargo-dist-v0.0.5-installer.sh | sh
```

Limitations/Caveats:

* Requires a well-defined [Artifact URL][artifact-url]
* Currently only really designed for "linux" and "macOS", and won't detect other platforms properly (and certainly won't play nice with things like nixOS).
* [Cannot detect situations where musl-based builds are appropriate][issue-musl] (static or dynamic)
* Relies on the user's installation of `tar` and `unzip` to unpack the files
* Relies on the the user's installation of `curl` or `wget` to fetch the files
* [Will throw out all files except for the binary, so the binary can't rely on assets included in the archive][issue-unpack-all]
* Cannot run any kind of custom install logic

In an ideal world all of these caveats improve (except for maybe relying on tar/unzip/curl/wget, that's kinda fundamental).



## Adding things to PATH

Here is a more fleshed out description of how the shell installer attempts to add the [install-path][config-install-path] to the user's PATH, and the limitations of that process.

The most fundamental limitation is that installers fundamentally cannot edit the PATH of the currently running shell (it's a parent process). Only an explicit `source some_file` (or the more portable `. some_file`) can do that. As such, it benefits an installer to try to install to a directory that will already be on PATH (such as [CARGO_HOME][cargo home]). Otherwise all we can do is prompt the user to run `source` themselves after the installer has run (or restart their shell to freshly source rcfiles).

The process we use to add [install-path][config-install-path] to the user's PATH is roughly the same process that rustup uses (hopefully making us harmonious with running rustup before/after one of our installer scripts). In the following description we will use `$install-path` as a placeholder for the path computed at install-time where the binaries get installed. Its actual value will likely look something like `$HOME/.myapp` or `$HOME/.cargo/bin`.

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



[issue-musl]: https://github.com/axodotdev/cargo-dist/issues/75
[issue-unpack-all]: https://github.com/axodotdev/cargo-dist/issues/307

[config-install-path]: ../reference/config.md#install-path

[archive]: ../artifacts/archives.md
[artifact-url]: ../reference/artifact-url.md

[cargo home]: https://doc.rust-lang.org/cargo/guide/cargo-home.html
