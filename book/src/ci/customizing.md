# Customizing GitHub Actions

<!-- toc -->

dist's generated CI configuration can be extended in several ways: it can be configured to install extra packages before the build begins, and it's possible to add extra jobs to run at specific lifecycle moments.


## Install extra packages

> since 0.4.0

Sometimes, you may need extra packages from the system package manager to be installed before in the builder before dist begins building your software. dist can do this for you by adding the `dependencies` setting to your dist config. When set, the packages you request will be fetched and installed in the step before `build`. Additionally, on macOS, the `cargo build` process will be wrapped in `brew bundle exec` to ensure that your dependencies can be found no matter where Homebrew placed them.

By default, we run Apple silicon (aarch64) builds for macOS on the `macos-13` runner, which is Intel-based. If your build process needs to link against C libraries from Homebrew using the `dependencies` feature, you will need to switch to an Apple silicon-native runner to ensure that you have access to Apple silicon-native dependencies from Homebrew. You can do this using the [custom runners][custom-runners] feature. Currently, `macos-14` is the oldest (and only) GitHub-provided runner for Apple silicon.

Sometimes, you may want to make sure your users also have these dependencies available when they install your software. If you use a package manager-based installer, dist has the ability to specify these dependencies. By default, dist will examine your program to try to detect which dependencies it thinks will be necessary. At the moment, [Homebrew][homebrew] is the only supported package manager installer. You can also specify these dependencies manually.

For more information, see the [configuration syntax][config-dependencies].

### Limitations

* Currently, the only supported package managers are Apt (Linux), Chocolatey (Windows) and Homebrew (macOS).

## Custom jobs

> since 0.3.0 (publish-jobs) and 0.7.0 (other steps)

dist's CI can be configured to call additional jobs on top of the ones it has builtin. Currently, we support adding extra jobs to the the following list of steps:

* [`plan-jobs`][config-plan] (the beginning of the build process)
* [`build-local-artifacts-jobs`][config-build-local]
* [`build-global-artifacts-jobs`][config-build-global]
* [`host-jobs`][config-host-jobs] (pre-publish)
* [`publish-jobs`][config-publish-jobs]
* [`post-announce-jobs`][config-post-announce] (after the release is created)

Custom jobs have access to the plan, produced via the "plan" step. This is a JSON document containing information about the project, planned steps, and its outputs. It's the same format contained as the "dist-manifest.json" that will be included with your release. You can use this in your custom jobs to obtain information about what will be built. For more details on the format of this file, see the [schema reference][schema].

To add a custom job, you need to follow two steps:

1. Define the new job as a reusable workflow using the standard method defined by your CI system. For GitHub actions, see the documentation on [reusable workflows](https://docs.github.com/en/actions/using-workflows/reusing-workflows#creating-a-reusable-workflow).
2. Add the name of your new workflow file to the appropriate array in your dist config, prefixed with a `./`. For example, if your job name is `.github/workflows/my-publish.yml`, you would write it like this:

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
    # dist exposes the plan from the plan step, as a JSON string,
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

Running `dist init` for your tool will update your GitHub Actions configuration to make use of the new reusable workflow during the publish step.

## Custom runners

> since 0.6.0

By default, dist uses the following runners:

* Linux (x86_64): `ubuntu-20.04`
* macOS (x86_64): `macos-13`
* macOS (Apple Silicon): `macos-13`
* Windows (x86_64): `windows-2019`

It's possible to configure alternate runners for these jobs, or runners for targets not natively supported by GitHub actions. To do this, use the [`github-custom-runners`][config-github-custom-runners] configuration setting in your dist config. Here's an example which adds support for Linux (aarch64) using runners from [Buildjet](https://buildjet.com/for-github-actions):

```toml
[workspace.metadata.dist.github-custom-runners]
aarch64-unknown-linux-gnu = "buildjet-8vcpu-ubuntu-2204-arm"
aarch64-unknown-linux-musl = "buildjet-8vcpu-ubuntu-2204-arm"
```

In addition to adding support for new targets, some users may find it useful to use this feature to fine-tune their builds for supported targets. For example, some projects may wish to build on a newer Ubuntu runner or alternate Linux distros, or may wish to opt into building for Apple Silicon from a native runner by using the `macos-14` runner. Here's an example which uses `macos-14` for native Apple Silicon builds:

```toml
[workspace.metadata.dist.github-custom-runners]
aarch64-apple-darwin = "macos-14"
```

### Build and upload artifacts on every pull request

> since 0.3.0

By default, dist will run the plan step on every pull request but won't perform a full release build. If these builds are turned on, the resulting pull request artifacts won't be uploaded to a release but will be available as a download from within the CI job. To enable this, select the "upload" option from the "check your release process in pull requests" question in `dist init` or set [the `pr-run-mode` key][config-pr-run-mode] to `"upload"` in `Cargo.toml`'s dist config. For example:

```toml
pr-run-mode = "upload"
```

## Advanced and esoteric features

These features are specialized to very particular usecases, but may be useful for some users.

### Customizing Build Setup

> since 0.20.0

This is an experimental feature.

In the event that installing platform dependencies using dist's system dependency feature
doesn't work for your needs, for example a build dependency for your project isn't provided by the
system's package manager, dist provides a method for injecting build steps into the
`build-local-artifacts` job to prepare the container.

To do this, use the [github-build-setup setting](../reference/config.md#github-build-setup) which
should be a path relative to your `.github/workflows/` directory, and which should point to a
`.yml` file containing the github workflow steps just as you would normally write them in a workflow.
(don't forget that leading `-`!)

For example, if you needed the Lua programming language installed you could update your `Cargo.toml` with the following:

```toml
[workspace.metadata.dist]
# ...
github-build-setup = "build-setup.yml"
```

An then include in the root of your repository a file named `.github/workflows/build-setup.yml` containing the
following.

```yml
- name: Install Lua
  uses: xpol/setup-lua@v1
  with:
    lua-version: "5.3"
- name: Check lua installation
  run: lua -e "print('hello world!')"
```

This would generate a `build-local-artifacts` job with the following modifications.

```yml
# ...
jobs:
# ...
  build-local-artifacts:
    # ...
    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive
      - name: Install Lua
        uses: xpol/setup-lua@v1
          with:
            lua-version: "5.3"
      - name: Check lua installation
        run: lua -e "print('hello world!')"
# ...
```

Notice that we include the steps right after the `actions/checkout` step meaning that we are doing
this as early in the build job as possible.

#### Limitations

##### Multi-line strings

Currently the use of [folding](https://yaml.org/spec/1.2.2/#813-folded-style) (`>`) and
[chomping](https://yaml.org/spec/1.2.2/#8112-block-chomping-indicator) (`-`) multi-line string
modifiers will probably generate a surprising outputs. This is particularly important for any
actions that use the `run` keyword and it is recomended to use the literal (`|`) string modifier for
multi-line strings.

### Bring your own release

> since 0.2.0

By default, dist will want to create its own GitHub Release and set the title/body with things like your CHANGELOG/RELEASES and some info about how to install the release. However if you have your own process for generating the contents of GitHub Release, we support that.

If you set [`create-release = false`](../reference/config.md#create-release) in your dist config, dist will assume a draft Github Release for the current git tag already exists with the title/body you want, and just upload artifacts to it. At the end of a successful publish it will undraft the GitHub Release for you.

### Publish GitHub Release To Another Repository

> since 0.14.0

You can change which repository a GitHub Release gets published to with the [github-releases-repo setting][config-github-releases-repo].

### Hand-editing release.yml

> since 0.3.0

The happy-path of dist has us completely managing release.yml, and since 0.3.0 we will actually consider it an error for there to be any edits or out of date information in release.yml.

If there's something that dist can't do that makes you want to hand-edit the file, we'd love to hear about it so that you can stay on the happy-path!

However we know you sometimes really need to do those hand-edits, so there is a way to opt into it. If you [set `allow-dirty = ["ci"]` in your dist config][config-allow-dirty], dist will stop trying to update the file and stop checking if it's out of date.

Although you're not "using dist wrong" if you do this, **be aware that you are losing access to a lot of the convenience and UX benefits of dist**. Every piece of documentation that says "just run dist init" may not work correctly, as a new feature may require the CI template to be updated. Even things as simple as "updating dist" will stop working.

We have put a lot of effort into minimizing those situations, with `plan` increasingly being responsible for dynamically computing what the CI should do, but that's not perfect, and there's no guarantees that future versions of dist won't completely change the way CI is structured.

### Fiddly build task settings

> since 0.0.1

Here's a grab-bag of more random settings you probably don't want to use, but exist in case you need them.

By default dist lets all the build tasks keep running even if one of them fails, to try to get you as much as possible when things go wrong. [`fail-fast = true` can be set to disable this][config-fail-fast].

By default dist breaks build tasks onto more machines than strictly necessary to create the maximum opportunities for concurrency and to increase fault-tolerance. For instance if you want to build for both arm64 macOS and x64 macOS, that *could* be done on the same machine, but we put it on two machines so they can be in parallel and succeed/fail independently. [`merge-tasks = true` can be set to disable this][config-merge-tasks].


[custom-runners]: #custom-runners

[config-dependencies]: ../reference/config.md#dependencies
[config-plan]: ../reference/config.md#plan-jobs
[config-allow-dirty]: ../reference/config.md#allow-dirty
[config-build-local]: ../reference/config.md#build-local-artifacts-jobs
[config-build-global]: ../reference/config.md#build-global-artifacts-jobs
[config-fail-fast]: ../reference/config.md#fail-fast
[config-github-custom-runners]: ../reference/config.md#github-custom-runners
[config-github-releases-repo]: ../reference/config.md#github-releases-repo
[config-host-jobs]: ../reference/config.md#host-jobs
[config-merge-tasks]: ../reference/config.md#merge-tasks
[config-publish-jobs]: ../reference/config.md#publish-jobs
[config-post-announce]: ../reference/config.md#post-announce-jobs
[config-pr-run-mode]: ../reference/config.md#pr-run-mode

[schema]: ../reference/schema.md

[homebrew]: ../installers/homebrew.md
