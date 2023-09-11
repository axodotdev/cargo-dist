# GitHub CI

> since 0.0.3

<!-- toc -->

The GitHub CI backend provides a "Release" Workflow that is triggered by pushing a tagged commit to your repository. It uses the tag to determine which packages you're trying to publish, and builds and uploads them to a GitHub Release.


## Setup

cargo-dist is currently very eager to setup the GitHub CI backend, so it's pretty easy to do! Most likely it was automatically setup the first time you ran `cargo dist init`. If you followed [the way-too-quickstart][quickstart], then you should also have it setup.


### Setup Step 1: set "repository" in your Cargo.toml

You probably already have it set, but if you don't, now's the time to do it. We need to know [the URL of your GitHub repository][artifact-url] for several features, and the next step will fail without it.


### Setup Step 2: run init and enable GitHub CI

Run `cargo dist init` on your project.

If you did the previous step, you should get prompted to "enable Github CI and Releases?", with the default answer being "yes". Choose yes.

You will also get prompted to "check your release process in pull requests?", with the default answer being "plan - run 'cargo dist plan' on PRs (recommended)". Choose that option.

Once init completes, some changes will be made to your project, **check all of them in**:

* `ci = ["github"]` should be added to `[workspace.metadata.dist]`
* `./github/workflows/release.yml` should be created, this is your Release Workflow


### Setup Step 3: you're done! (time to test)

See [the quickstart's testing guide][testing] for the various testing options.

The easiest testing option for this is to open a pull-request for everything you checked in -- it should run the `plan` step of your release CI as part of the PR.



## Advanced Usage

Here are some more advanced things you can do with GitHub CI.


### Build and upload artifacts on every pull request

> since 0.3.0

By default, cargo-dist will run the plan step on every pull request but won't perform a full release build. If these builds are turned on, the resulting pull request artifacts won't be uploaded to a release but will be available as a download from within the CI job. To enable this, select the "upload" option from the "check your release process in pull requests" question in `cargo-dist-init` or set [the `pr-run-mode` key][config-pr-run-mode] to `"upload"` in `Cargo.toml`'s cargo-dist config. For example:

```toml
pr-run-mode = "upload"
```


### Bring your own release

> since 0.2.0

By default, cargo-dist will want to create its own GitHub Release and set the title/body with things like your CHANGELOG/RELEASES and some info about how to install the release. However if you have your own process for generating the contents of GitHub Release, we support that.

If you set `create-release = false` in your cargo-dist config, cargo-dist will assume a draft Github Release for the current git tag already exists with the title/body you want, and just upload artifacts to it. At the end of a successful publish it will undraft the GitHub Release for you.



### Custom jobs

> since 0.3.0

cargo-dist's CI can be configured to call additional jobs on top of the ones it has builtin. Currently, we support adding extra jobs to the publish step; in the future, we'll allow extending all of the lifecycle steps of the CI workflow. To add one, you need to follow two steps:

1. Define the new job as a reusable workflow using the standard method defined by your CI system. For GitHub actions, see the documentation on [reusable workflows](https://docs.github.com/en/actions/using-workflows/reusing-workflows#creating-a-reusable-workflow).
2. Add the name of your new workflow file to the `publish-jobs` array in your `Cargo.toml`'s cargo-dist config, prefixed with a `./`. For example, if your job name is `.github/workflows/my-publish.yml`, you would write it like this:

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



### Install extra packages

> since 0.4.0

Sometimes, you may need extra packages from the system package manager to be installed before in the builder before cargo-dist begins building your software. Cargo-dist can do this for you by adding the `dependencies` setting to your `Cargo.toml`. When set, the packages you request will be fetched and installed in the step before `build`. Additionally, on macOS, the `cargo build` process will be wrapped in `brew bundle exec` to ensure that your dependencies can be found no matter where Homebrew placed them. For more information, see the [configuration syntax][config-dependencies].

#### Limitations

* Currently, the only supported package managers are Apt (Linux), Chocolatey (Windows) and Homebrew (macOS).
* On macOS, system dependencies can only be enabled for x86_64 builds because GitHub does not provide Apple Silicon runners and Apple Silicon Homebrew can't be installed on x86_64 macOS.



### Hand-editing release.yml

> since 0.3.0

The happy-path of cargo-dist has us completely managing release.yml, and since 0.3.0 we will actually consider it an error for there to be any edits or out of date information in release.yml.

If there's something that cargo-dist can't do that makes you want to hand-edit the file, we'd love to hear about it so that you can stay on the happy-path!

However we know you sometimes really need to do those hand-edits, so there is a way to opt into it. If you [set `allow-dirty = ["ci"]` in your cargo-dist config][config-allow-dirty], cargo-dist will stop trying to update the file and stop checking if it's out of date.

Although you're not "using cargo-dist wrong" if you do this, **be aware that you are losing access to a lot of the convenience and UX benefits of cargo-dist**. Every piece of documentation that says "just run cargo dist init" may not work correctly, as a new feature may require the CI template to be updated. Even things as simple as "updating cargo-dist" will stop working.

We have put a lot of effort into minimizing those situations, with `plan` increasingly being responsible for dynamically computing what the CI should do, but that's not perfect, and there's no guarantees that future versions of cargo-dist won't completely change the way CI is structured.


### Fiddly build task settings

> since 0.0.1

Here's a grab-bag of more random settings you probably don't want to use, but exist in case you need them.

By default cargo-dist lets all the build tasks keep running even if one of them fails, to try to get you as much as possible when things go wrong. [`fail-fast = true` can be set to disable this][config-fail-fast].

By default cargo-dist breaks build tasks onto more machines than strictly necessary to create the maximum opportunities for concurrency and to increase fault-tolerance. For instance if you want to build for both arm64 macOS and x64 macOS, that *could* be done on the same machine, but we put it on two machines so they can be in parallel and succeed/fail independently. [`merge-tasks = true` can be set to disable this][config-merge-tasks].



[config-fail-fast]: ../reference/config.md#fail-fast
[config-merge-tasks]: ../reference/config.md#merge-tasks
[config-allow-dirty]: ../reference/config.md#allow-dirty
[config-pr-run-mode]: ../reference/config.md#pr-run-mode
[config-dependencies]: ../reference/config.md#dependencies

[artifact-url]: ../reference/artifact-url.md#github
[quickstart]: ../way-too-quickstart.md
[testing]: ../way-too-quickstart.md#test-it-out
