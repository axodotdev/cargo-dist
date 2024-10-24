# JavaScript Quickstart

<!-- toc -->

So you've written a JavaScript CLI application and you'd like to distribute standalone executables so your users don't need to install nodejs or npm, without having to write your own CI or installers? Well, good news, dist can do just that! This guide will help you get up and running as quickly as possible.


## Initial dist Setup

🔧 This feature requires some extra setup that will be builtin in the future, please let us know if it doesn't work for you!

This is based on the [axolotlsay-bun example project](https://github.com/axodotdev/axolotlsay-bun).


### Preparing Your JS Project

We will be using [bun build --compile](https://bun.sh/docs/bundler/executables) to generate standalone executables for an npm project. In the future this will be more builtin, but for now we're going to need to teach your npm package to install bun and build itself. To do this we're going to add bun as a dev-dependency of our application and add a "dist" [npm script](https://docs.npmjs.com/cli/v10/using-npm/scripts) that runs `bun build` on itself.

[Here's what the changes look like in axolotlsay-bun](https://github.com/axodotdev/axolotlsay-bun/commit/8aeebf8dfd91527352b5d9afe0146cd752028f19).


#### Adding Bun As A Dev Dependency

To make it easy for anyone working on our package to get the Right version of bun and use it, we can install it as an npm dev-dependency like so:

```sh
npm i bun --save-dev
```

Your package.json should now have something like this:

```json
  "devDependencies": {
    "bun": "^1.x.x"
  }
```

#### Adding A dist Script

We want it to be easy for anyone to run our bun build on any platform, so add [a script called "dist" to our package.json](https://docs.npmjs.com/cli/v10/using-npm/scripts):

```json
  "scripts": {
    "predist": "npm install",
    "dist": "node dist.js"
  },
```

We run `npm install` in "predist" to ensure dev-dependencies like bun are installed for anyone who runs the "dist" script. The name "dist" here is important, as dist will be looking for it. However the file it runs can have any name/location. Here we're calling it "dist.js", and it contains the following:

```js
// you might need to change this path to your package.json
const { bin } = require("./package.json");
const execSync = require('child_process').execSync;

// Compute the target we're building for
const bunTargets = {
    "x86_64-pc-windows-msvc": "bun-windows-x64",
    "aarch64-apple-darwin": "bun-darwin-arm64",
    "x86_64-apple-darwin": "bun-darwin-x64",
    "aarch64-unknown-linux-gnu": "bun-linux-arm64",
    "x86_64-unknown-linux-gnu": "bun-linux-x64"
}
const distTarget = process.env.CARGO_DIST_TARGET || process.env.DIST_TARGET;
if (!distTarget) {
    throw "DIST_TARGET isn't set, so we don't know what platform to build!"
}
const bunTarget = bunTargets[distTarget];
if (!bunTarget) {
    throw `To the the best of our knowledge, bun does not support building for ${distTarget}`;
}
const binExt = distTarget.includes("windows") ? ".exe" : "";

// setup bun
execSync("bun install");

// for each binary, run bun
for (binName of Object.keys(bin)) {
    const binScript = bin[binName];
    const binPath = `${binName}${binExt}`;
    execSync(`bun build ${binScript} --compile --target ${bunTarget} --outfile ${binPath}`);
}
```

Ideally you won't have to customize this script at all (except maybe the relative path to package.json on the first line), because it reads your package.json and determines what to do for you. In particular it requires you to have [a "bin" field in your package.json](https://docs.npmjs.com/cli/v10/configuring-npm/package-json#bin):

```json
  "bin": {
    "axolotlsay-bun": "index.js"
  },
```

While you're here, you should also make sure you've set required fields like:

* "name"
* "version"
* "repository" set

So your package.json should at a minimum look something like this:

```json
{
  "name": "axolotlsay-bun",
  "version": "0.4.0",
  "repository": "github:axodotdev/axolotlsay-hybrid",
  "bin": {
    "axolotlsay-bun": "index.js"
  },
  "scripts": {
    "predist": "npm install",
    "dist": "node dist.js"
  },
  "devDependencies": {
    "bun": "^1.x.x"
  }
}
```

#### Adding dist-workspace.toml

dist uses a custom configuration format called `dist-workspace.toml`, written in the [TOML][toml] format. dist can manage most of your settings for you, or find them in your package.json, but we need to tell it that we're making a JS project.

Create a file named `dist-workspace.toml` in the root of your repository. These are the entire contents of the file (you supply the path):

```toml
[workspace]
members = ["npm:relative/path/to/your/package/"]
```

(If your project is in the root, this may just be `members = ["npm:./"]`)


### First Init

Once you've done that and also [installed dist][install], we can ask dist to generate the rest of its configuration for us: just run `dist init`, and answer all the questions it asks you. This command interactively walks you through configuration options, **and should be run again whenever you want to change your settings or want to update dist**.

Just to really emphasize that: [`dist init` is designed to be rerun over and over, and will preserve your settings while handling any necessary updates and migrations. Always Be Initing](../updating.md).

Since this is a quickstart, we'll pass `--yes` to auto-accept all defaults on our first setup!

```sh
# setup dist in your project (--yes to accept defaults)
dist init --yes
git add .
git commit -am 'chore: wow shiny new dist CI!'
```

**It's very common for `dist init` to return an error about the "repository" URLs set in your package.json. If this happens, no work will be lost.** You can just follow the instructions in the error and rerun `dist init` again and it will pick up where you left off.**

This one-time setup will:

* create your dist config in `dist-workspace.toml`
* generate CI for orchestrating itself in `.github/workflows/release.yml`


### Adding Installers

> 🚨🚨🚨 VERY IMPORTANT 🚨🚨🚨
>
> dist supports "npm installers" and "npm publishes" but these refer to a feature that wraps your prebuilt binaries in an npm package that fetches them, and is [not (YET) a thing for actual native javascript projects](https://github.com/axodotdev/cargo-dist/issues/1169)!

The most common reason to update dist or mess with its config is to add a new [installer][], which is basically our blanket term for anything more fancy than [tarballs][] (curl-sh scripts, npm packages, msi installers, ...).

You can skip this step for now and just test out the basics the initial setup gives you. Each individual [installer][] should have a guide that assumes you did the initial setup.

The tl;dr of those guides is "run `dist init` again, select the installer you want to add, and fill in any extra details that are needed".



## Test It Out

There are a several ways to test out dist before committing to running a proper release:

1. build for the current platform (`dist build`)
2. check what CI will build (`dist plan`)
3. check the release process on pull-requests




### Build For The Current Platform

```sh
dist build
```

![Running "dist build" on a project, resulting in the application getting built and bundled into a .zip, and an "installer.ps1" script getting generated. Paths to these files are printed along with some metadata.][quickstart-build]

The [build command][build] will by default try to build things for the computer you're running it on. So if you run it on linux you might get a `tar.xz` containing your binary and an installer.sh, but if you run it on windows you might get a `zip` and an installer.ps1.

dist will then spit out paths to the files it created, so you can inspect their contents and try running them (**note that installer scripts probably won't be locally runnable, because they will try to fetch their binaries from GitHub**).





### Check What CI Will Build

```sh
dist plan
```

![Running "dist plan" on a project, producing a full printout of the tarballs/zips that will be produced for all platforms (mac, linux, windows), and all installers (shell, powershell)][quickstart-plan]

The [plan command][plan] should be running the exact same logic that dist's generated CI will run, but without actually building anything. This lets you quickly check what cutting a new release will produce. It will also try to catch any inconsistencies that could make the CI error out.




### Check The Release Process On Pull-Requests

By default we run the "plan" step of your release CI on every pull-request so that we can catch breakage to your release process as early as possible. This will work even for a pull-request that sets up dist for the first time, so you can be confident you're landing something that works.

You can also crank this up by setting `pr-run-mode = "upload"`, which will run all the build steps as well, and upload the results to the PR's Workflow Summary as an "artifacts.zip". This is great for making sure the windows build works even if you only have a linux machine, or vice-versa. Although you should probably only keep it on temporarily, as it's very slow and wasteful to build all those shippable artifacts for every PR.



## Cut A Release (Trigger Github CI)

dist largely doesn't care about the details of how you prepare your release, and doesn't yet provide tools to streamline it. All it cares about is you getting your release branch into the state you want, and then pushing a properly formatted git tag like "v0.1.0". Here's a super bare-bones release process where we're releasing by just pushing a bunch of stuff to main branch (but it would work just as well with PRs and release branches):

```sh
# <manually update the version of your package, run tests, etc>

# commit and push to main (can be done with a PR)
git commit -am "release: version 0.1.0"
git push

# actually push the tag up (this triggers dist's CI)
git tag v0.1.0
git push --tags
```

The important parts are that you update the packages you want to release/announce to the desired version and push a git tag with that version.

At this point you're done! The generated CI script should pick up the ball and create a Github Release with all your builds over the next few minutes!




[quickstart-build]: ../img/quickstart-build.png
[quickstart-plan]: ../img/quickstart-plan.png

[guide]: ../workspaces/index.md
[install]: ../install.md
[cargo-release-guide]: ../workspaces/cargo-release-guide.md
[artifact-modes]: ../reference/concepts.md#artifact-modes-selecting-artifacts
[installer]: ../installers/index.md
[tarballs]: ../artifacts/archives.md
[build]: ../reference/cli.md#dist-build
[plan]: ../reference/cli.md#dist-plan

[toml]: https://toml.io/en/
