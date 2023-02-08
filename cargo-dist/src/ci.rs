//! CI script generation
//!
//! In the future this may get split up into submodules.

use std::fs::File;

use camino::Utf8PathBuf;
use miette::{IntoDiagnostic, WrapErr};
use tracing::warn;

use crate::InstallerStyle;

const GITHUB_CI_TRIGGER: &str = r###"# CI that:
#
# * checks for a Git Tag that looks like a release ("v1.2.0")
# * creates a Github Release™
# * builds binaries/packages with cargo-dist
# * uploads those packages to the Github Release™
#
# Note that the Github Release™ will be created before the packages,
# so there will be a few minutes where the release has no packages
# and then they will slowly trickle in, possibly failing. To make
# this more pleasant we mark the release as a "draft" until all
# artifacts have been successfully uploaded. This allows you to
# choose what to do with partial successes and avoids spamming
# anyone with notifications before the release is actually ready.
name: Release

permissions:
  contents: write

# This task will run whenever you push a git tag that looks like
# a version number. We just look for `v` followed by at least one number
# and then whatever. so `v1`, `v1.0.0`, and `v1.0.0-prerelease` all work.
#
# If there's a prerelease-style suffix to the version then the Github Release™
# will be marked as a prerelease (handled by taiki-e/create-gh-release-action).
#
# Note that when generating links to uploaded artifacts, cargo-dist will currently
# assume that your git tag is always v{VERSION} where VERSION is the version in
# the published package's Cargo.toml (this is the default behaviour of cargo-release).
# In the future this may be made more robust/configurable.
on:
  push:
    tags:
      - v[0-9]+.*

env:"###;

const GITHUB_CI_CREATE_RELEASE: &str = r###"
jobs:
  # Create the Github Release™ so the packages have something to be uploaded to
  create-release:
    runs-on: ubuntu-latest
    outputs:
      tag: ${{ steps.create-gh-release.outputs.computed-prefix }}${{ steps.create-gh-release.outputs.version }}
    steps:
      - uses: actions/checkout@v3
      - id: create-gh-release
        uses: taiki-e/create-gh-release-action@v1
        with:
          draft: true
          # (required) GitHub token for creating GitHub Releases.
          token: ${{ secrets.GITHUB_TOKEN }}


  # Build and packages all the things
  upload-artifacts:
    needs: create-release
    strategy:
      matrix:
        # For these target platforms
        include:"###;

const GITHUB_CI_ARTIFACT_TASKS1: &str = r###"    runs-on: ${{ matrix.os }}
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    steps:
      - uses: actions/checkout@v3
      - name: Install Rust
        run: rustup update stable && rustup default stable
      - name: Install cargo-dist
        run: ${{ matrix.install-dist }}
      - name: Run cargo-dist
        # This logic is a bit janky because it's trying to be a polyglot between
        # powershell and bash since this will run on windows, macos, and linux!
        # The two platforms don't agree on how to talk about env vars but they
        # do agree on 'cat' and '$()' so we use that to marshal values between commmands.
        run: |
          # Actually do builds and make zips and whatnot
          cargo dist --target=${{ matrix.target }} --output-format=json > dist-manifest.json
          echo "dist ran successfully"
          cat dist-manifest.json
          # Parse out what we just built and upload it to the Github Release™
          cat dist-manifest.json | jq --raw-output ".releases[].artifacts[].path" > uploads.txt
          echo "uploading..."
          cat uploads.txt
          gh release upload ${{ needs.create-release.outputs.tag }} $(cat uploads.txt)
          echo "uploaded!"

  # Compute and upload the manifest for everything
  upload-manifest:
    needs: create-release
    runs-on: ubuntu-latest
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    steps:
      - uses: actions/checkout@v3
      - name: Install Rust
        run: rustup update stable && rustup default stable
      - name: Install cargo-dist"###;
const GITHUB_CI_ARTIFACT_TASKS2: &str = r###"      - name: Run cargo-dist manifest
        run: |
          # Generate a manifest describing everything
          cargo dist manifest --no-local-paths --output-format=json $ALL_CARGO_DIST_TARGET_ARGS $ALL_CARGO_DIST_INSTALLER_ARGS > dist-manifest.json
          echo "dist manifest ran successfully"
          cat dist-manifest.json
          # Upload the manifest to the Github Release™
          gh release upload ${{ needs.create-release.outputs.tag }} dist-manifest.json
          echo "uploaded manifest!"
          # Edit the Github Release™ title/body to match what cargo-dist thinks it should be
          CHANGELOG_TITLE=$(cat dist-manifest.json | jq --raw-output ".releases[].changelog_title")
          cat dist-manifest.json | jq --raw-output ".releases[].changelog_body" > new_dist_changelog.md
          gh release edit ${{ needs.create-release.outputs.tag }} --title="$CHANGELOG_TITLE" --notes-file=new_dist_changelog.md
          echo "updated release notes!""###;

const GITHUB_CI_INSTALLERS: &str = r###"      - name: Run cargo-dist --installer=...
        run: |
          # Run cargo dist with --no-builds to get agnostic artifacts like installers
          cargo dist --output-format=json --no-builds $ALL_CARGO_DIST_INSTALLER_ARGS > dist-manifest.json
          echo "dist ran successfully"
          cat dist-manifest.json
          # Grab the installers that were generated and upload them.
          # This filter is working around the fact that --no-builds is kinds hacky
          # and still makes/reports malformed zips that we don't want to upload.
          cat dist-manifest.json | jq --raw-output '.releases[].artifacts[] | select(.kind == "installer") | .path' > uploads.txt
          echo "uploading..."
          cat uploads.txt
          gh release upload ${{ needs.create-release.outputs.tag }} $(cat uploads.txt)
          echo "uploaded installers!""###;

const GITHUB_CI_FINISH_RELEASE: &str = r###"
  # Mark the Github Release™ as a non-draft now that everything has succeeded!
  publish-release:
    needs: [create-release, upload-artifacts, upload-manifest]
    runs-on: ubuntu-latest
    env:
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    steps:
      - uses: actions/checkout@v3
      - name: mark release as non-draft
        run: |
          gh release edit ${{ needs.create-release.outputs.tag }} --draft=false
"###;

/// Generate CI for Github
///
/// This actually creates a file and writes to disk!
pub fn generate_github_ci(
    workspace_dir: &Utf8PathBuf,
    targets: &[String],
    installers: &[InstallerStyle],
) -> Result<(), miette::Report> {
    const GITHUB_CI_DIR: &str = ".github/workflows/";
    const GITHUB_CI_FILE: &str = "release.yml";

    // FIXME: should we try to avoid clobbering old files..?
    let ci_dir = workspace_dir.join(GITHUB_CI_DIR);
    let ci_file = ci_dir.join(GITHUB_CI_FILE);
    std::fs::create_dir_all(&ci_dir)
        .into_diagnostic()
        .wrap_err("Failed to create ci dir")?;
    let mut file = File::create(ci_file)
        .into_diagnostic()
        .wrap_err("Failed to create ci file")?;
    write_github_ci(&mut file, targets, installers)
        .into_diagnostic()
        .wrap_err("Failed to write to CI file")?;
    Ok(())
}

fn write_github_ci(
    f: &mut File,
    targets: &[String],
    installers: &[InstallerStyle],
) -> Result<(), std::io::Error> {
    use std::io::Write;

    writeln!(f, "{GITHUB_CI_TRIGGER}")?;

    // Write out target args
    let mut target_args = Vec::new();
    for target in targets {
        write!(&mut target_args, "--target={target} ")?;
    }
    let target_args = String::from_utf8(target_args).unwrap();
    writeln!(f, "  ALL_CARGO_DIST_TARGET_ARGS: {target_args}")?;

    // Write out the installer args
    let mut installer_args = Vec::new();
    for installer in installers {
        let installer = match installer {
            InstallerStyle::GithubShell => "github-shell",
            InstallerStyle::GithubPowershell => "github-powershell",
        };
        write!(&mut installer_args, "--installer={installer} ")?;
    }

    // If no installer args are present, add two single quotes to keep the YAML valid
    // See https://github.com/axodotdev/cargo-dist/issues/101.
    if installer_args.is_empty() {
        write!(&mut installer_args, "''")?;
    }

    let installer_args = String::from_utf8(installer_args).unwrap();
    writeln!(f, "  ALL_CARGO_DIST_INSTALLER_ARGS: {installer_args}")?;

    // Write out the current version
    let dist_version = env!("CARGO_PKG_VERSION");
    //writeln!(f, "  CARGO_DIST_VERSION: v{dist_version}")?;
    let install_unix = format!("curl --proto '=https' --tlsv1.2 -L -sSf https://github.com/axodotdev/cargo-dist/releases/download/v{dist_version}/installer.sh | sh");
    let install_windows = format!("irm 'https://github.com/axodotdev/cargo-dist/releases/download/v{dist_version}/installer.ps1' | iex");
    // writeln!(f, "  CARGO_DIST_INSTALL_UNIX: {install_unix}")?;
    // writeln!(f, "  CARGO_DIST_INSTALL_WINDOWS: {install_windows}")?;

    writeln!(f, "{GITHUB_CI_CREATE_RELEASE}")?;

    for target in targets {
        let Some(os) = github_os_for_target(target) else {
            warn!("skipping generating ci for {target} (no idea what github os should build this)");
            continue;
        };
        writeln!(f, "        - target: {target}")?;
        writeln!(f, "          os: {os}")?;
        let install_cmd = if target.contains("windows") {
            &install_windows
        } else {
            &install_unix
        };
        writeln!(f, "          install-dist: {install_cmd}")?;
    }

    writeln!(f, "{GITHUB_CI_ARTIFACT_TASKS1}")?;
    writeln!(f, "        run: {install_unix}")?;
    writeln!(f, "{GITHUB_CI_ARTIFACT_TASKS2}")?;

    if !installers.is_empty() {
        writeln!(f, "{GITHUB_CI_INSTALLERS}")?;
    }

    writeln!(f, "{GITHUB_CI_FINISH_RELEASE}")?;
    Ok(())
}

fn github_os_for_target(target: &str) -> Option<&'static str> {
    // We want to default to older runners to minimize the places
    // where random system dependencies can creep in and be very
    // recent. This helps with portability!
    if target.contains("linux") {
        Some("ubuntu-20.04")
    } else if target.contains("apple") {
        Some("macos-11")
    } else if target.contains("windows") {
        Some("windows-2019")
    } else {
        None
    }
}
