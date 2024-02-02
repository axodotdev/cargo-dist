# Customizing CI

<!-- toc -->

cargo-dist's generated CI configuration can be extended in several ways: it can be configured to install extra packages before the build begins, and it's possible to add extra jobs to run at specific lifecycle moments.

In the past, you may have customized cargo-dist's generated CI configuration and used the `allow-dirty = ["ci"]` configuration option. With these new customization options, you may well not need to directly hand-edit cargo-dist's config any longer; we encourate migrating to these new tools.


## Install extra packages

> since 0.4.0

Sometimes, you may need extra packages from the system package manager to be installed before in the builder before cargo-dist begins building your software. Cargo-dist can do this for you by adding the `dependencies` setting to your `Cargo.toml`. When set, the packages you request will be fetched and installed in the step before `build`. Additionally, on macOS, the `cargo build` process will be wrapped in `brew bundle exec` to ensure that your dependencies can be found no matter where Homebrew placed them.

Sometimes, you may want to make sure your users also have these dependencies available when they install your software. If you use a package manager-based installer, cargo-dist has the ability to specify these dependencies. By default, cargo-dist will examine your program to try to detect which dependencies it thinks will be necessary. At the moment, [Homebrew][homebrew] is the only supported package manager installer. You can also specify these dependencies manually.

For more information, see the [configuration syntax][config-dependencies].


## Custom jobs

> since 0.3.0 (publish-jobs) and 0.7.0 (other steps)

cargo-dist's CI can be configured to call additional jobs on top of the ones it has builtin. Currently, we support adding extra jobs to the the following list of steps:

* [`plan-jobs`][config-plan] (the beginning of the build process)
* [`build-local-artifacts-jobs`][config-build-local]
* [`build-global-artifacts-jobs`][config-build-global]
* [`host-jobs`][config-host-jobs] (pre-publish)
* [`publish-jobs`][config-publish-jobs]
* [`post-announce-jobs`][config-post-announce] (after the release is created)

Custom jobs have access to the plan, produced via the "plan" step. This is a JSON document containing information about the project, planned steps, and its outputs. It's the same format contained as the "dist-manifest.json" that will be included with your release. You can use this in your custom jobs to obtain information about what will be built. For more details on the format of this file, see the [schema reference][schema].

To add a custom job, you need to follow two steps:

1. Define the new job as a reusable workflow using the standard method defined by your CI system. For GitHub actions, see the documentation on [reusable workflows](https://docs.github.com/en/actions/using-workflows/reusing-workflows#creating-a-reusable-workflow).
2. Add the name of your new workflow file to the appropriate array in your `Cargo.toml`'s cargo-dist config, prefixed with a `./`. For example, if your job name is `.github/workflows/my-publish.yml`, you would write it like this:

```toml
publish-jobs = ["./my-publish"]
```

Here's an example reusable workflow written using GitHub Actions. It won't do any real publishing, just echo text to the CI output. First, create a file named `.github/workflows/publish-greeter.yml` with these contents:

```yaml
name: Greeter

on:
  # Defining workflow_call means that this workflow can be called from
  # your main workflow job
  workflow_call:
    # cargo-dist exposes the plan from the plan step, as a JSON string,
    # to your job if it needs it
    inputs:
      plan:
        required: true
        type: string

jobs:
  greeter:
    runs-on: ubuntu-latest
    # This is optional; it exposes the plan to your job as an environment variable
    env:
      PLAN: ${{ inputs.plan }}
    steps:
      - name: Step 1
        run: |
          echo "Hello!"
          echo "Plan is: ${PLAN}"
```

Then, add the following to your `publish-jobs` array:

```toml
publish-jobs = ["./publish-greeter"]
```

Running `cargo-dist init` for your tool will update your GitHub Actions configuration to make use of the new reusable workflow during the publish step.

## Custom runners

(since 0.6.0)

By default, cargo-dist uses the following runners:

* Linux (x86_64): `ubuntu-20.04`
* macOS (x86_64): `macos-12`
* macOS (Apple Silicon): `macos-14`
* Windows (x86_64): `windows-2019`

It's possible to configure alternate runners for these jobs, or runners for targets not natively supported by GitHub actions. To do this, use the [`github-custom-runners`](config-github-custom-runners) configuration setting in `Cargo.toml`. Here's an example which adds support for Linux (aarch64) using runners from [Buildjet](https://buildjet.com/for-github-actions):

```toml
[workspace.metadata.dist.github-custom-runners]
aarch64-unknown-linux-gnu = "buildjet-8vcpu-ubuntu-2204-arm"
aarch64-unknown-linux-musl = "buildjet-8vcpu-ubuntu-2204-arm"
```

In addition to adding support for new targets, some users may find it useful to use this feature to fine-tune their builds for supported targets. For example, some projects may wish to build on a newer Ubuntu runner or alternate Linux distros, or may wish to opt into cross-compiling for Apple Silicon from an Intel-based runner.

[config-dependencies]: ../reference/config.md#dependencies
[config-plan]: ../reference/config.md#plan-jobs
[config-build-local]: ../reference/config.md#build-local-artifacts-jobs
[config-build-global]: ../reference/config.md#build-global-artifacts-jobs
[config-github-custom-runners]: ../reference/config.md#github-custom-runners
[config-host-jobs]: ../reference/config.md#host-jobs
[config-publish-jobs]: ../reference/config.md#publish-jobs
[config-post-announce]: ../reference/config.md#post-announce-jobs

[schema]: ../reference/schema.md

[homebrew]: ../installers/homebrew.md
