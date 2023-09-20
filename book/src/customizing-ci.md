# Customizing CI

Out of the box, cargo-dist's CI configuration is designed to cover the majority of usecases. We provide some tools to customize how it works for your project.

## Build and upload artifacts on every pull request

> since 0.3.0

By default, cargo-dist will run the plan step on every pull request but won't perform a full release build. If these builds are turned on, the resulting pull request artifacts won't be uploaded to a release but will be available as a download from within the CI job. To enable this, select the "upload" option from the "check your release process in pull requests" question in `cargo-dist-init` or set the `pr-run-mode` key to `"upload"` in `Cargo.toml`'s cargo-dist config. For example:

```toml
pr-run-mode = "upload"
```

## Custom jobs

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
