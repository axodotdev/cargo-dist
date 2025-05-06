# Usage

axo's installers are designed to require no end user input - however, in many
cases, end users like the ability to tailor their installation experience. For
these users, our installers allow several inputs which give the user more
control over their installation experience via configuration. Most of these are
implemented using environment variables, however several are also available as
arguments to the installer scripts themselves.

This documentation is targeted at developers using `dist` to distribute their apps. If you expect your users to be interested in any of these features, we recommend incorporating the relevant information into your own installation documentation. If there's any additional documentation you'd find helpful here, please feel free to [open an issue][open-issue]!

Several of these options were available in earlier releases of `dist`, but they
are formally stabilized as of `0.23.0`.

## Install location

> since 0.1.0

One of the most important jobs of an installer is to place the necessary
artifacts in the correct location on the target machine. `dist` allows you to
configure this for your particular needs. Depending on your setup, the following
options are available for end users to further configure this location:

- `${app name}_INSTALL_DIR`

    > Replace `{app name}` with the name of the application. To transform the
    > app name to the env var, replace any spaces or hyphens with an underscore
    > and then put it in all caps. You can double check this transform by
    > comparing the `install_dir_env_var` value in your `dist-manifest.json`.

    This environment variable tells the installer what to use as the "base"
    directory for the installation task. This may not necessarily be the exact
    directory your binaries are installed to based on your install layout. For
    example, this directory may have `./bin` appended to it.

    This environment variable is also known as `CARGO_DIST_FORCE_INSTALL_DIR`.

- `$HOME`

    This environment variable is sourced if you use the `CARGO_HOME` or
    `~/sub/dir` install location options. For more information, see the [documentation for this feature][install-path].

- `${custom env var}`

    You can use a custom environment variable to specify your install
    location. If you do, that variable will be sourced during the install task. For more information, see the [documentation for this feature][install-path].

- `$CARGO_HOME`

    This environment variable is sourced if you use the `CARGO_HOME` install
    location. For more information, see the [documentation for this feature][install-path].

## `$PATH`

> since 0.1.0, path modification options updated in 0.23.0

`$PATH` is an environment variable that pre-exists on nearly all systems and
lists locations to look for executables in. This is what allows you to call a
program by its name instead of needing to call it by it's specific location on
the file system.

When you use axo installers, we do the heavy lifting of ensuring that your
application is available "on PATH" to your end users. On Linux and macOS, we do this by editing shell dotfiles; on Windows, we do this by editing the `Environment.Path` registry key. However, there are
circumstances where this is not desirable, and so we provide the ability to
skip this setup step.

- ### `$PATH`

    This environment variable is both sourced and modified in the standard mode
    of an installation. We source this variable to see if the installation
    location is already on PATH and we will modify it if it is not.

- ### `$PATH` modification

    If you do not want your PATH to be modified you can use the `INSTALLER_NO_MODIFY_PATH` environment variable to configure your installation experience.

- ### `$GITHUB_PATH`

    If an installer detects the presence of this environment variable
    (signalling that it is running in a GitHub Actions context) our installers
    will modify this environment variable to ensure that all installed
    applications are immediately available on PATH.

## Artifact location

> since 0.25.0

Some folks, particularly those working in security-sensitive business environments,
may need to mirror artifacts within a private network. `dist` enables this usecase
by allowing end users to customize the URL that artifacts are fetched from:

- `${app name}_INSTALLER_GITHUB_BASE_URL`
- `${app name}_INSTALLER_GHE_BASE_URL`

> Replace `{app name}` with the name of the application. To transform the
> app name to the env var, replace any spaces or hyphens with an underscore
> and then put it in all caps. You can double check this transform by
> comparing the `install_dir_env_var` value in your `dist-manifest.json`.

These environment variables enable you to specify both a base URL and a URL
structure to the installer and updater of a project that distributes with `dist`.
When set, installers will fetch from URL constructed based on the value you set
here.

When setting up your mirror you'll need to both mirror the artifacts *and* provide
an endpoint that indexes the available releases (so that the updater can work).

To minimize complexity for both us and our end users, we have standardized our
requested API structure expectations to match either:

- Github.com, or
    - Public artifact URLs: https://{CUSTOM}/owner/repo/releases/download/version/artifact-name
    - Releases API: https://api.{CUSTOM}/repos/owner/repo/releases/latest ([docs](https://docs.github.com/en/rest/releases/releases?apiVersion=2022-11-28))
- Github Enterprise
    - Public artifact URLs: https://{CUSTOM}/owner/repo/releases/download/version/artifact-name
    - Releases API: https://{CUSTOM}/api/v3/repos/owner/repo/releases/latest ([docs](https://docs.github.com/en/enterprise-server@3.14/rest/releases/releases?apiVersion=2022-11-28))

`dist` is eager to support enterprise level features like this- so if you have questions
or related feature requests, please join our [Discord](https://discord.gg/XAFG6xSZ) or send
us an email at hello@axo.dev.

## Receipt

> since 0.9.0

When you use axo to distribute your application, in addition to installers, you
may also enable an updater - either integrated into your application using a
library or as a standalone binary shipped alongside your application.

The updater functionality relies on knowing how your application was originally
installed and where. To keep track of this information, the installer writes a
receipt that is read by the the updater.

You can configure this receipt writing using the following options:

- Shell: The `$HOME` environment variable is sourced to write the receipt to
  `$HOME/.config/{app name}`.
- PowerShell: The `$LOCALAPPDATA` environment variable is sourced to write the
  receipt to `$LOCALAPPDATA/{app name}`.

## Unmanaged mode

> since 0.23.0

This is intended for users installing in ephemeral environments such as CI and disables several features that are unneeded in those environments. To use it, set the `${app name}_UNMANAGED_INSTALL` environment variable to the desired installation path.

> Replace `{app name}` with the name of the application. To transform the
> app name to the env var, replace any spaces or hyphens with an underscore
> and then put it in all caps. You can double check this transform by
> comparing the `install_dir_env_var` value in your `dist-manifest.json`.

Enabling this mode does the following things:

* Disables updater-related tooling, including install receipt creation
* Disables modification of the user's `PATH`, including modification of dotfiles
* Forces a flat installation layout, installing all files into a single directory

## Debug

As you work with axo's installers, you will, despite everyone's best efforts,
find yourself debugging an issue. You can use the following options:

- ### Message level
    - Shell: `$INSTALLER_PRINT_VERBOSE`, `-v, --verbose` and `$INSTALLER_PRINT_QUIET`, `-q, --quiet``
    - PowerShell: `-Verbose`

[install-path]: ../reference/config.md#install-path
[open-issue]: https://github.com/axodotdev/cargo-dist/issues/new
