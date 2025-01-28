use crate::{platform::MinGlibcVersion, METADATA_DIST};
use crate::config::{InstallPathStrategy, SystemDependencies};
use crate::config::v1::{
    artifacts::archives::ArchiveLayer,
    artifacts::ArtifactLayer,
    builds::BuildLayer,
    ci::CiLayer,
    hosts::HostLayer,
    layer::BoolOr,
    publishers::PublisherLayer,
    TomlLayer,
};
use crate::config::v1::installers::{
    homebrew::HomebrewInstallerLayer, msi::MsiInstallerLayer, npm::NpmInstallerLayer,
    pkg::PkgInstallerLayer, powershell::PowershellInstallerLayer,
    shell::ShellInstallerLayer, CommonInstallerLayer, InstallerLayer,
};
use axoasset::toml_edit;

use crate::config::v1::layer::BoolOrOptExt;

/// Update a workspace toml-edit document with the current DistMetadata value
pub fn apply_dist_to_workspace_toml(
    workspace_toml: &mut toml_edit::DocumentMut,
    meta: &TomlLayer,
) {
    let metadata = workspace_toml.as_item_mut();
    apply_dist_to_metadata(metadata, meta);
}

/// Ensure [dist] has the given values
pub fn apply_dist_to_metadata(metadata: &mut toml_edit::Item, meta: &TomlLayer) {
    let dist_metadata = &mut metadata[METADATA_DIST];

    // If there's no table, make one
    if !dist_metadata.is_table() {
        *dist_metadata = toml_edit::table();
    }

    // Apply formatted/commented values
    let table = dist_metadata.as_table_mut().unwrap();

    // This is intentionally written awkwardly to make you update this
    let TomlLayer {
        config_version,
        dist_version,
        dist_url_override,
        dist,
        allow_dirty,
        targets,
        artifacts,
        builds,
        ci,
        hosts,
        installers,
        publishers,
    } = &meta;

    let installers = &Some(apply_default_install_path(installers));

    apply_optional_value(
        table,
        "config-version",
        "# The configuration version to use (valid options: 1)\n",
        Some(config_version.to_string()),
    );

    apply_optional_value(
        table,
        "dist-version",
        "# The preferred dist version to use in CI (Cargo.toml SemVer syntax)\n",
        dist_version.as_ref().map(|v| v.to_string()),
    );

    apply_optional_value(
        table,
        "dist-url-override",
        "# A URL to use to install `cargo-dist` (with the installer script)\n",
        dist_url_override.as_ref().map(|v| v.to_string()),
    );

    apply_optional_value(
        table,
        "dist",
        "# Whether the package should be distributed/built by dist (defaults to true)\n",
        *dist,
    );

    apply_string_list(
        table,
        "allow-dirty",
        "# Skip checking whether the specified configuration files are up to date\n",
        allow_dirty.as_ref(),
    );

    apply_string_list(
        table,
        "targets",
        "# Target platforms to build apps for (Rust target-triple syntax)\n",
        targets.as_ref(),
    );

    apply_artifacts(table, artifacts);
    apply_builds(table, builds);
    apply_ci(table, ci);
    apply_hosts(table, hosts);
    apply_installers(table, installers);
    apply_publishers(table, publishers);

    // TODO(migration): make sure all of these are handled
    /*


    apply_optional_value(
        table,
        "checksum",
        "# Checksums to generate for each App\n",
        checksum.map(|c| c.ext().as_str()),
    );

    apply_optional_value(
        table,
        "merge-tasks",
        "# Whether to run otherwise-parallelizable tasks on the same machine\n",
        *merge_tasks,
    );

    apply_optional_value(
        table,
        "fail-fast",
        "# Whether failing tasks should make us give up on all other tasks\n",
        *fail_fast,
    );

    apply_optional_value(
        table,
        "cache-builds",
        "# Whether builds should try to be cached in CI\n",
        *cache_builds,
    );

    apply_optional_value(
        table,
        "build-local-artifacts",
        "# Whether CI should include auto-generated code to build local artifacts\n",
        *build_local_artifacts,
    );

    apply_optional_value(
        table,
        "dispatch-releases",
        "# Whether CI should trigger releases with dispatches instead of tag pushes\n",
        *dispatch_releases,
    );

    apply_optional_value(
        table,
        "release-branch",
        "# Trigger releases on pushes to this branch instead of tag pushes\n",
        release_branch.as_ref(),
    );

    apply_optional_value(
        table,
        "create-release",
        "# Whether dist should create a Github Release or use an existing draft\n",
        *create_release,
    );

    apply_optional_value(
        table,
        "github-release",
        "# Which phase dist should use to create the GitHub release\n",
        github_release.as_ref().map(|a| a.to_string()),
    );

    apply_optional_value(
        table,
        "github-releases-repo",
        "# Publish GitHub Releases to this repo instead\n",
        github_releases_repo.as_ref().map(|a| a.to_string()),
    );

    apply_optional_value(
        table,
        "github-releases-submodule-path",
        "# Read the commit to be tagged from the submodule at this path\n",
        github_releases_submodule_path
            .as_ref()
            .map(|a| a.to_string()),
    );

    apply_string_list(
        table,
        "plan-jobs",
        "# Plan jobs to run in CI\n",
        plan_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "local-artifacts-jobs",
        "# Local artifacts jobs to run in CI\n",
        local_artifacts_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "global-artifacts-jobs",
        "# Global artifacts jobs to run in CI\n",
        global_artifacts_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "host-jobs",
        "# Host jobs to run in CI\n",
        host_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "publish-jobs",
        "# Publish jobs to run in CI\n",
        publish_jobs.as_ref(),
    );

    apply_string_list(
        table,
        "post-announce-jobs",
        "# Post-announce jobs to run in CI\n",
        post_announce_jobs.as_ref(),
    );

    apply_optional_value(
        table,
        "publish-prereleases",
        "# Whether to publish prereleases to package managers\n",
        *publish_prereleases,
    );

    apply_optional_value(
        table,
        "force-latest",
        "# Always mark releases as latest, ignoring semver semantics\n",
        *force_latest,
    );

    apply_optional_value(
        table,
        "pr-run-mode",
        "# Which actions to run on pull requests\n",
        pr_run_mode.as_ref().map(|m| m.to_string()),
    );

    apply_optional_value(
        table,
        "github-attestations",
        "# Whether to enable GitHub Attestations\n",
        *github_attestations,
    );

    apply_string_or_list(
        table,
        "hosting",
        "# Where to host releases\n",
        hosting.as_ref(),
    );

    apply_optional_value(
        table,
        "tag-namespace",
        "# A prefix git tags must include for dist to care about them\n",
        tag_namespace.as_ref(),
    );

    apply_optional_value(
        table,
        "install-updater",
        "# Whether to install an updater program\n",
        *install_updater,
    );

    apply_optional_value(
        table,
        "always-use-latest-updater",
        "# Whether to always use the latest updater instead of a specific known-good version\n",
        *always_use_latest_updater,
    );

    apply_optional_value(
        table,
        "display",
        "# Whether to display this app's installers/artifacts in release bodies\n",
        *display,
    );

    apply_optional_value(
        table,
        "display-name",
        "# Custom display name to use for this app in release bodies\n",
        display_name.as_ref(),
    );



    */

    // Finalize the table
    table.decor_mut().set_prefix("\n# Config for 'dist'\n");
}

fn apply_default_install_path(installers: &Option<InstallerLayer>) -> InstallerLayer {
    let mut installers = installers.clone().unwrap_or_default();

    // Forcibly inline the default install_path if not specified,
    // and if we've specified a shell or powershell installer
    let install_path = if installers.common.install_path.is_none()
        && !(installers.shell.is_none_or_false() || installers.powershell.is_none_or_false())
    {
        Some(InstallPathStrategy::default_list())
    } else {
        installers.common.install_path.clone()
    };

    installers.common.install_path = install_path;
    installers
}

fn apply_artifacts(table: &mut toml_edit::Table, artifacts: &Option<ArtifactLayer>) {
    let Some(artifacts) = artifacts else {
        return;
    };
    let Some(artifacts_table) = table.get_mut("artifacts") else {
        return;
    };
    let toml_edit::Item::Table(artifacts_table) = artifacts_table else {
        panic!("Expected [dist.artifacts] to be a table");
    };

    // TODO(migration): implement this

    apply_artifacts_archives(artifacts_table, &artifacts.archives);

    apply_optional_value(
        artifacts_table,
        "source-tarball",
        "# Generate and dist a source tarball\n",
        artifacts.source_tarball,
    );

    // TODO(migration): implement dist.artifacts.extra.
    /*
    apply_optional_value(
        artifacts_table,
        "extra",
        "# Any extra artifacts, and their build scripts\n",
        artifacts.extra,
    );
    */

    apply_optional_value(
        artifacts_table,
        "checksum",
        "# The checksum format to generate\n",
        artifacts.checksum.map(|cs| cs.to_string()),
    );

    // Finalize the table
    artifacts_table
        .decor_mut()
        .set_prefix("\n# Artifact configuration for dist\n");
}

fn apply_artifacts_archives(
    artifacts_table: &mut toml_edit::Table,
    archives: &Option<ArchiveLayer>,
) {
    let Some(archives) = archives else {
        return;
    };
    let Some(archives_table) = artifacts_table.get_mut("archives") else {
        return;
    };
    let toml_edit::Item::Table(archives_table) = archives_table else {
        panic!("Expected [dist.artifacts.archives] to be a table");
    };

    apply_string_list(
        archives_table,
        "include",
        "# Extra static files to include in each App (path relative to this Cargo.toml's dir)\n",
        archives.include.as_ref(),
    );

    apply_optional_value(
        archives_table,
        "auto-includes",
        "# Whether to auto-include files like READMEs, LICENSEs, and CHANGELOGs (default true)\n",
        archives.auto_includes,
    );

    apply_optional_value(
        archives_table,
        "windows-archive",
        "# The archive format to use for windows builds (defaults .zip)\n",
        archives.windows_archive.map(|a| a.ext()),
    );

    apply_optional_value(
        archives_table,
        "unix-archive",
        "# The archive format to use for non-windows builds (defaults .tar.xz)\n",
        archives.unix_archive.map(|a| a.ext()),
    );

    apply_string_or_list(
        archives_table,
        "package-libraries",
        "# Which kinds of built libraries to include in the final archives\n",
        archives.package_libraries.as_ref(),
    );
}

fn apply_builds(table: &mut toml_edit::Table, builds: &Option<BuildLayer>) {
    let Some(builds) = builds else {
        // Nothing to do.
        return;
    };
    let Some(builds_table) = table.get_mut("builds") else {
        // Nothing to do.
        return;
    };
    let toml_edit::Item::Table(builds_table) = builds_table else {
        panic!("Expected [dist.builds] to be a table");
    };

    apply_optional_value(
        builds_table,
        "ssldotcom-windows-sign",
        "# Whether we should sign Windows binaries using ssl.com",
        builds
            .ssldotcom_windows_sign
            .as_ref()
            .map(|p| p.to_string()),
    );

    apply_optional_value(
        builds_table,
        "macos-sign",
        "# Whether to sign macOS executables\n",
        builds.macos_sign,
    );

    apply_cargo_builds(builds_table, builds);
    apply_system_dependencies(builds_table, builds.system_dependencies.as_ref());

    apply_optional_min_glibc_version(
        builds_table,
        "min-glibc-version",
        "# The minimum glibc version supported by the package (overrides auto-detection)\n",
        builds.min_glibc_version.as_ref(),
    );

    apply_optional_value(
        builds_table,
        "omnibor",
        "# Whether to use omnibor-cli to generate OmniBOR Artifact IDs\n",
        builds.omnibor,
    );

    // Finalize the table
    builds_table
        .decor_mut()
        .set_prefix("\n# Build configuration for dist\n");
}

fn apply_cargo_builds(builds_table: &mut toml_edit::Table, builds: &BuildLayer) {
    if let Some(BoolOr::Bool(b)) = builds.cargo {
        // If it was set as a boolean, simply set it as a boolean and return.
        apply_optional_value(builds_table,
            "cargo",
            "# Whether dist should build cargo projects\n# (Use the table format of [dist.builds.cargo] for more nuanced config!)\n",
            Some(b),
        );
        return;
    }

    let Some(BoolOr::Val(ref cargo_builds)) = builds.cargo else {
        return;
    };

    let mut possible_table = toml_edit::table();
    let cargo_builds_table = builds_table.get_mut("cargo").unwrap_or(&mut possible_table);

    let toml_edit::Item::Table(cargo_builds_table) = cargo_builds_table else {
        panic!("Expected [dist.builds.cargo] to be a table")
    };

    apply_optional_value(
        cargo_builds_table,
        "rust-toolchain-version",
        "# The preferred Rust toolchain to use in CI (rustup toolchain syntax)\n",
        cargo_builds.rust_toolchain_version.as_deref(),
    );

    apply_optional_value(
        cargo_builds_table,
        "msvc-crt-static",
        "# Whether +crt-static should be used on msvc\n",
        cargo_builds.msvc_crt_static,
    );

    apply_optional_value(
        cargo_builds_table,
        "precise-builds",
        "# Build only the required packages, and individually\n",
        cargo_builds.precise_builds,
    );

    apply_string_list(
        cargo_builds_table,
        "features",
        "# Features to pass to cargo build\n",
        cargo_builds.features.as_ref(),
    );

    apply_optional_value(
        cargo_builds_table,
        "default-features",
        "# Whether default-features should be enabled with cargo build\n",
        cargo_builds.default_features,
    );

    apply_optional_value(
        cargo_builds_table,
        "all-features",
        "# Whether to pass --all-features to cargo build\n",
        cargo_builds.all_features,
    );

    apply_optional_value(
        cargo_builds_table,
        "cargo-auditable",
        "# Whether to embed dependency information using cargo-auditable\n",
        cargo_builds.cargo_auditable,
    );

    apply_optional_value(
        cargo_builds_table,
        "cargo-cyclonedx",
        "# Whether to use cargo-cyclonedx to generate an SBOM\n",
        cargo_builds.cargo_cyclonedx,
    );

    // Finalize the table
    cargo_builds_table
        .decor_mut()
        .set_prefix("\n# How dist should build Cargo projects\n");
}

fn apply_system_dependencies(
    builds_table: &mut toml_edit::Table,
    system_dependencies: Option<&SystemDependencies>,
) {
    let Some(system_dependencies) = system_dependencies else {
        // Nothing to do.
        return;
    };

    // TODO(migration): implement this
}

fn apply_ci(table: &mut toml_edit::Table, ci: &Option<CiLayer>) {
    let Some(ci_table) = table.get_mut("ci") else {
        // Nothing to do.
        return;
    };
    let toml_edit::Item::Table(ci_table) = ci_table else {
        panic!("Expected [dist.ci] to be a table");
    };

    // TODO(migration): implement this

    // Finalize the table
    ci_table
        .decor_mut()
        .set_prefix("\n# CI configuration for dist\n");
}

fn apply_hosts(table: &mut toml_edit::Table, hosts: &Option<HostLayer>) {
    let Some(hosts_table) = table.get_mut("hosts") else {
        // Nothing to do.
        return;
    };
    let toml_edit::Item::Table(hosts_table) = hosts_table else {
        panic!("Expected [dist.hosts] to be a table");
    };

    // TODO(migration): implement this

    // Finalize the table
    hosts_table
        .decor_mut()
        .set_prefix("\n# Hosting configuration for dist\n");
}

fn apply_installers(table: &mut toml_edit::Table, installers: &Option<InstallerLayer>) {
    let Some(installers) = installers else {
        return;
    };
    let Some(installers_table) = table.get_mut("installers") else {
        return;
    };
    let toml_edit::Item::Table(installers_table) = installers_table else {
        panic!("Expected [dist.installers] to be a table");
    };

    apply_installers_common(installers_table, &installers.common);

    if let Some(homebrew) = &installers.homebrew {
        match homebrew {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "homebrew",
                    "# Whether to build a Homebrew installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_homebrew(installers_table, v);
            }
        }
    }

    if let Some(msi) = &installers.msi {
        match msi {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "msi",
                    "# Whether to build an MSI installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_msi(installers_table, v);
            }
        }
    }

    if let Some(npm) = &installers.npm {
        match npm {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "npm",
                    "# Whether to build an NPM installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_npm(installers_table, v);
            }
        }
    }

    if let Some(powershell) = &installers.powershell {
        match powershell {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "powershell",
                    "# Whether to build a PowerShell installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_powershell(installers_table, v);
            }
        }
    }

    if let Some(shell) = &installers.shell {
        match shell {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "shell",
                    "# Whether to build a Shell installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_shell(installers_table, v);
            }
        }
    }

    if let Some(pkg) = &installers.pkg {
        match pkg {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "pkg",
                    "\n# Configuration for the Mac .pkg installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_pkg(installers_table, v);
            }
        }
    }

    // installer.updater: Option<Bool>
    // installer.always_use_latest_updater: Option<bool>
    apply_optional_value(
        installers_table,
        "updater",
        "# Whether to install an updater program alongside the software\n",
        installers.updater,
    );

    apply_optional_value(
        installers_table,
        "always-use-latest-updater",
        "# Whether to always use the latest updater version instead of a fixed version\n",
        installers.always_use_latest_updater,
    );

    // Finalize the table
    installers_table
        .decor_mut()
        .set_prefix("\n# Installer configuration for dist\n");
}

fn apply_installers_common(table: &mut toml_edit::Table, common: &CommonInstallerLayer) {
    apply_string_or_list(
        table,
        "install-path",
        "# Path that installers should place binaries in\n",
        common.install_path.as_ref(),
    );

    apply_optional_value(
        table,
        "install-success-msg",
        "# Custom message to display on successful install\n",
        common.install_success_msg.as_deref(),
    );

    apply_string_or_list(
        table,
        "install-libraries",
        "# Which kinds of packaged libraries to install\n",
        common.install_libraries.as_ref(),
    );

    // / Aliases to install binaries as
    // TODO(migration): handle `pub bin_aliases: Option<SortedMap<String, Vec<String>>>`
}

fn apply_installers_homebrew(
    installers_table: &mut toml_edit::Table,
    homebrew: &HomebrewInstallerLayer,
) {
    let Some(homebrew_table) = installers_table.get_mut("homebrew") else {
        return;
    };
    let toml_edit::Item::Table(homebrew_table) = homebrew_table else {
        panic!("Expected [dist.installers.homebrew] to be a table");
    };

    apply_installers_common(homebrew_table, &homebrew.common);

    apply_optional_value(
        homebrew_table,
        "tap",
        "# A GitHub repo to push Homebrew formulas to\n",
        homebrew.tap.clone(),
    );

    apply_optional_value(
        homebrew_table,
        "formula",
        "# Customize the Homebrew formula name\n",
        homebrew.formula.clone(),
    );

    // Finalize the table
    homebrew_table
        .decor_mut()
        .set_prefix("\n# Configure the built Homebrew installer\n");
}

fn apply_installers_msi(installers_table: &mut toml_edit::Table, msi: &MsiInstallerLayer) {
    let Some(msi_table) = installers_table.get_mut("msi") else {
        return;
    };
    let toml_edit::Item::Table(msi_table) = msi_table else {
        panic!("Expected [dist.installers.msi] to be a table");
    };

    apply_installers_common(msi_table, &msi.common);

    // There are no items under MsiInstallerConfig aside from `msi.common`.

    msi_table
        .decor_mut()
        .set_prefix("\n# Configure the built MSI installer\n");
}

fn apply_installers_npm(installers_table: &mut toml_edit::Table, npm: &NpmInstallerLayer) {
    let Some(npm_table) = installers_table.get_mut("npm") else {
        return;
    };
    let toml_edit::Item::Table(npm_table) = npm_table else {
        panic!("Expected [dist.installers.npm] to be a table");
    };

    apply_installers_common(npm_table, &npm.common);

    apply_optional_value(
        npm_table,
        "package",
        "# The npm package should have this name\n",
        npm.package.as_deref(),
    );

    apply_optional_value(
        npm_table,
        "scope",
        "# A namespace to use when publishing this package to the npm registry\n",
        npm.scope.as_deref(),
    );
}

fn apply_installers_powershell(
    installers_table: &mut toml_edit::Table,
    powershell: &PowershellInstallerLayer,
) {
    let Some(powershell_table) = installers_table.get_mut("powershell") else {
        return;
    };
    let toml_edit::Item::Table(powershell_table) = powershell_table else {
        panic!("Expected [dist.installers.powershell] to be a table");
    };

    apply_installers_common(powershell_table, &powershell.common);

    // TODO(migration): implement this (similar to shell)
}

fn apply_installers_shell(installers_table: &mut toml_edit::Table, shell: &ShellInstallerLayer) {
    let Some(shell_table) = installers_table.get_mut("shell") else {
        return;
    };
    let toml_edit::Item::Table(shell_table) = shell_table else {
        panic!("Expected [dist.installers.shell] to be a table");
    };

    apply_installers_common(shell_table, &shell.common);

    // TODO(migration): implement this
}

fn apply_installers_pkg(installers_table: &mut toml_edit::Table, pkg: &PkgInstallerLayer) {
    let Some(pkg_table) = installers_table.get_mut("pkg") else {
        return;
    };
    let toml_edit::Item::Table(pkg_table) = pkg_table else {
        panic!("Expected [dist.installers.pkg] to be a table");
    };

    apply_installers_common(pkg_table, &pkg.common);

    apply_optional_value(
        pkg_table,
        "identifier",
        "# A unique identifier, in tld.domain.package format\n",
        pkg.identifier.clone(),
    );

    apply_optional_value(
        pkg_table,
        "install-location",
        "# The location to which software should be installed (defaults to /usr/local)\n",
        pkg.install_location.clone(),
    );

    // Finalize the table
    pkg_table
        .decor_mut()
        .set_prefix("\n# Configuration for the Mac .pkg installer\n");
}

fn apply_publishers(table: &mut toml_edit::Table, publishers: &Option<PublisherLayer>) {
    let Some(publishers_table) = table.get_mut("publishers") else {
        return;
    };
    let toml_edit::Item::Table(publishers_table) = publishers_table else {
        panic!("Expected [dist.publishers] to be a table");
    };

    // TODO(migration): implement this

    // Finalize the table
    publishers_table
        .decor_mut()
        .set_prefix("\n# Publisher configuration for dist\n");
}

/// Update the toml table to add/remove this value
///
/// If the value is Some we will set the value and hang a description comment off of it.
/// If the given key already existed in the table, this will update it in place and overwrite
/// whatever comment was above it. If the given key is new, it will appear at the end of the
/// table.
///
/// If the value is None, we delete it (and any comment above it).
fn apply_optional_value<I>(table: &mut toml_edit::Table, key: &str, desc: &str, val: Option<I>)
where
    I: Into<toml_edit::Value>,
{
    if let Some(val) = val {
        table.insert(key, toml_edit::value(val));
        if let Some(mut key) = table.key_mut(key) {
            key.leaf_decor_mut().set_prefix(desc)
        }
    } else {
        table.remove(key);
    }
}

/// Same as [`apply_optional_value`][] but with a list of items to `.to_string()`
fn apply_string_list<I>(table: &mut toml_edit::Table, key: &str, desc: &str, list: Option<I>)
where
    I: IntoIterator,
    I::Item: std::fmt::Display,
{
    if let Some(list) = list {
        let items = list.into_iter().map(|i| i.to_string()).collect::<Vec<_>>();
        let array: toml_edit::Array = items.into_iter().collect();
        // FIXME: Break the array up into multiple lines with pretty formatting
        // if the list is "too long". Alternatively, more precisely toml-edit
        // the existing value so that we can preserve the user's formatting and comments.
        table.insert(key, toml_edit::Item::Value(toml_edit::Value::Array(array)));
        if let Some(mut key) = table.key_mut(key) {
            key.leaf_decor_mut().set_prefix(desc)
        }
    } else {
        table.remove(key);
    }
}

/// Same as [`apply_string_list`][] but when the list can be shorthanded as a string
fn apply_string_or_list<I>(table: &mut toml_edit::Table, key: &str, desc: &str, list: Option<I>)
where
    I: IntoIterator,
    I::Item: std::fmt::Display,
{
    if let Some(list) = list {
        let items = list.into_iter().map(|i| i.to_string()).collect::<Vec<_>>();
        if items.len() == 1 {
            apply_optional_value(table, key, desc, items.into_iter().next())
        } else {
            apply_string_list(table, key, desc, Some(items))
        }
    } else {
        table.remove(key);
    }
}

/// Similar to [`apply_optional_value`][] but specialized to `MinGlibcVersion`, since we're not able to work with structs dynamically
fn apply_optional_min_glibc_version(
    table: &mut toml_edit::Table,
    key: &str,
    desc: &str,
    val: Option<&MinGlibcVersion>,
) {
    if let Some(min_glibc_version) = val {
        let new_item = &mut table[key];
        let mut new_table = toml_edit::table();
        if let Some(new_table) = new_table.as_table_mut() {
            for (target, version) in min_glibc_version {
                new_table.insert(target, toml_edit::Item::Value(version.to_string().into()));
            }
            new_table.decor_mut().set_prefix(desc);
        }
        new_item.or_insert(new_table);
    } else {
        table.remove(key);
    }
}
