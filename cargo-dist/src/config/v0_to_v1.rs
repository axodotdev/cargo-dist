//! conversion layer from v0 config to v1 config

use artifacts::archives::ArchiveLayer;
use artifacts::ArtifactLayer;
use builds::cargo::CargoBuildLayer;
use builds::{BuildLayer, CommonBuildLayer};
use ci::github::GithubCiLayer;
use ci::{CiLayer, CommonCiLayer};
use hosts::github::GithubHostLayer;
use hosts::{CommonHostLayer, HostLayer};
use installers::homebrew::HomebrewInstallerLayer;
use installers::npm::NpmInstallerLayer;
use installers::pkg::PkgInstallerLayer;
use installers::{CommonInstallerLayer, InstallerLayer};
use layer::BoolOr;
use publishers::{CommonPublisherLayer, PublisherLayer};

use super::v0::DistMetadata;
use super::{v1::*, CiStyle, HostingStyle, InstallerStyle, JobStyle, MacPkgConfig, PublishStyle};

impl DistMetadata {
    /// Convert the v0 config format to v1
    pub fn to_toml_layer(&self, is_global: bool) -> TomlLayer {
        let DistMetadata {
            cargo_dist_version,
            cargo_dist_url_override,
            rust_toolchain_version,
            dist,
            ci,
            pr_run_mode,
            allow_dirty,
            installers,
            install_success_msg,
            tap,
            formula,
            system_dependencies,
            targets,
            include,
            auto_includes,
            msvc_crt_static,
            windows_archive,
            unix_archive,
            npm_package,
            npm_scope,
            checksum,
            precise_builds,
            merge_tasks,
            fail_fast,
            cache_builds,
            build_local_artifacts,
            dispatch_releases,
            release_branch,
            install_path,
            features,
            default_features,
            all_features,
            plan_jobs,
            local_artifacts_jobs,
            global_artifacts_jobs,
            source_tarball,
            host_jobs,
            publish_jobs,
            post_announce_jobs,
            publish_prereleases,
            force_latest,
            create_release,
            github_releases_repo,
            github_releases_submodule_path,
            github_release,
            github_action_commits,
            ssldotcom_windows_sign,
            macos_sign,
            mac_pkg_config,
            github_attestations,
            hosting,
            extra_artifacts,
            github_custom_runners,
            github_custom_job_permissions,
            bin_aliases,
            tag_namespace,
            install_updater,
            always_use_latest_updater,
            display,
            display_name,
            package_libraries,
            install_libraries,
            github_build_setup,
            min_glibc_version,
            binaries,
            cargo_auditable,
            cargo_cyclonedx,
            omnibor,
        } = self.clone();

        // Archives
        let needs_archive_layer = include.is_some()
            || auto_includes.is_some()
            || windows_archive.is_some()
            || unix_archive.is_some()
            || package_libraries.is_some()
            || binaries.is_some();
        let archive_layer = needs_archive_layer.then_some(ArchiveLayer {
            include,
            auto_includes,
            windows_archive,
            unix_archive,
            package_libraries,
            binaries,
        });
        let needs_artifacts = archive_layer.is_some()
            || source_tarball.is_some()
            || extra_artifacts.is_some()
            || checksum.is_some();
        let artifacts_layer = needs_artifacts.then_some(ArtifactLayer {
            archives: archive_layer,
            source_tarball,
            extra: extra_artifacts,
            checksum,
        });

        // Builds
        let needs_cargo_build_layer = rust_toolchain_version.is_some()
            || precise_builds.is_some()
            || features.is_some()
            || default_features.is_some()
            || all_features.is_some()
            || cargo_auditable.is_some()
            || cargo_cyclonedx.is_some();
        let cargo_layer = needs_cargo_build_layer.then_some(BoolOr::Val(CargoBuildLayer {
            common: CommonBuildLayer::default(),
            rust_toolchain_version,
            precise_builds,
            features,
            default_features,
            all_features,
            msvc_crt_static,
            cargo_auditable,
            cargo_cyclonedx,
        }));
        let needs_build_layer = cargo_layer.is_some()
            || system_dependencies.is_some()
            || ssldotcom_windows_sign.is_some()
            || msvc_crt_static.is_some()
            || min_glibc_version.is_some()
            || omnibor.is_some();
        let build_layer = needs_build_layer.then_some(BuildLayer {
            common: CommonBuildLayer {},
            ssldotcom_windows_sign,
            macos_sign,
            system_dependencies,
            cargo: cargo_layer,
            generic: None,
            min_glibc_version,
            omnibor,
        });

        // CI
        let github_ci_layer = list_to_bool_layer(is_global, &ci, CiStyle::Github, || {
            if github_custom_runners.is_some()
                || github_custom_job_permissions.is_some()
                || github_build_setup.is_some()
                || github_action_commits.is_some()
            {
                Some(GithubCiLayer {
                    common: CommonCiLayer::default(),
                    runners: github_custom_runners,
                    permissions: github_custom_job_permissions,
                    build_setup: github_build_setup,
                    action_commits: github_action_commits,
                })
            } else {
                None
            }
        });
        let has_github_ci = github_ci_layer.is_some();
        let custom_publish_jobs = publish_jobs.as_ref().map(|jobs| {
            jobs.iter()
                .filter_map(|p| {
                    if let PublishStyle::User(path) = p {
                        Some(JobStyle::User(path.to_owned()))
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        });
        let custom_publish_jobs = if custom_publish_jobs
            .as_deref()
            .unwrap_or_default()
            .is_empty()
        {
            None
        } else {
            custom_publish_jobs
        };
        let needs_ci_layer = github_ci_layer.is_some()
            || merge_tasks.is_some()
            || fail_fast.is_some()
            || cache_builds.is_some()
            || build_local_artifacts.is_some()
            || dispatch_releases.is_some()
            || release_branch.is_some()
            || pr_run_mode.is_some()
            || tag_namespace.is_some()
            || plan_jobs.is_some()
            || local_artifacts_jobs.is_some()
            || global_artifacts_jobs.is_some()
            || host_jobs.is_some()
            || custom_publish_jobs.is_some();
        let ci_layer = needs_ci_layer.then_some(CiLayer {
            common: CommonCiLayer {
                merge_tasks,
                fail_fast,
                cache_builds,
                build_local_artifacts,
                dispatch_releases,
                release_branch,
                pr_run_mode,
                tag_namespace,
                plan_jobs,
                build_local_jobs: local_artifacts_jobs,
                build_global_jobs: global_artifacts_jobs,
                host_jobs,
                publish_jobs: custom_publish_jobs,
                post_announce_jobs,
            },
            github: github_ci_layer,
        });

        // hosts
        let mut github_host_layer =
            list_to_bool_layer(is_global, &hosting, HostingStyle::Github, || {
                if create_release.is_some()
                    || github_releases_repo.is_some()
                    || github_releases_submodule_path.is_some()
                    || github_release.is_some()
                    || github_attestations.is_some()
                {
                    Some(GithubHostLayer {
                        common: CommonHostLayer::default(),
                        create: create_release,
                        repo: github_releases_repo,
                        submodule_path: github_releases_submodule_path.map(|p| p.into()),
                        during: github_release,
                        attestations: github_attestations,
                    })
                } else {
                    None
                }
            });
        let axodotdev_host_layer =
            list_to_bool_layer(is_global, &hosting, HostingStyle::Axodotdev, || None);
        if github_host_layer.is_none() && axodotdev_host_layer.is_none() && has_github_ci {
            github_host_layer = Some(BoolOr::Bool(true));
        }

        let needs_host_layer = github_host_layer.is_some()
            || axodotdev_host_layer.is_some()
            || force_latest.is_some()
            || display.is_some()
            || display_name.is_some();
        let host_layer = needs_host_layer.then_some(HostLayer {
            common: CommonHostLayer {},
            github: github_host_layer,
            axodotdev: axodotdev_host_layer,
            force_latest,
            display,
            display_name,
        });

        // installers
        let homebrew_installer_layer =
            list_to_bool_layer(is_global, &installers, InstallerStyle::Homebrew, || {
                if tap.is_some() || formula.is_some() {
                    Some(HomebrewInstallerLayer {
                        common: CommonInstallerLayer::default(),
                        tap,
                        formula,
                    })
                } else {
                    None
                }
            });
        let npm_installer_layer =
            list_to_bool_layer(is_global, &installers, InstallerStyle::Npm, || {
                if npm_package.is_some() || npm_scope.is_some() {
                    Some(NpmInstallerLayer {
                        common: CommonInstallerLayer::default(),
                        package: npm_package,
                        scope: npm_scope,
                    })
                } else {
                    None
                }
            });
        let msi_installer_layer =
            list_to_bool_layer(is_global, &installers, InstallerStyle::Msi, || None);
        let pkg_installer_layer =
            list_to_bool_layer(is_global, &installers, InstallerStyle::Pkg, || {
                let MacPkgConfig {
                    identifier,
                    install_location,
                } = mac_pkg_config?;
                Some(PkgInstallerLayer {
                    common: CommonInstallerLayer::default(),
                    identifier,
                    install_location,
                })
            });
        let powershell_installer_layer =
            list_to_bool_layer(is_global, &installers, InstallerStyle::Powershell, || None);
        let shell_installer_layer =
            list_to_bool_layer(is_global, &installers, InstallerStyle::Shell, || None);
        let needs_installer_layer = homebrew_installer_layer.is_some()
            || npm_installer_layer.is_some()
            || msi_installer_layer.is_some()
            || powershell_installer_layer.is_some()
            || shell_installer_layer.is_some()
            || pkg_installer_layer.is_some()
            || install_path.is_some()
            || install_success_msg.is_some()
            || install_libraries.is_some()
            || bin_aliases.is_some()
            || install_updater.is_some()
            || always_use_latest_updater.is_some();
        let installer_layer = needs_installer_layer.then_some(InstallerLayer {
            common: CommonInstallerLayer {
                install_path,
                install_success_msg,
                install_libraries,
                bin_aliases,
            },
            homebrew: homebrew_installer_layer,
            msi: msi_installer_layer,
            npm: npm_installer_layer,
            powershell: powershell_installer_layer,
            shell: shell_installer_layer,
            pkg: pkg_installer_layer,
            updater: install_updater,
            always_use_latest_updater,
        });

        // publish
        let homebrew_publisher_layer =
            list_to_bool_layer(is_global, &publish_jobs, PublishStyle::Homebrew, || None);
        let npm_publisher_layer =
            list_to_bool_layer(is_global, &publish_jobs, PublishStyle::Npm, || None);
        let needs_publisher_layer = homebrew_publisher_layer.is_some()
            || npm_publisher_layer.is_some()
            || publish_prereleases.is_some();
        let publisher_layer = needs_publisher_layer.then_some(PublisherLayer {
            common: CommonPublisherLayer {
                prereleases: publish_prereleases,
            },
            homebrew: homebrew_publisher_layer,
            npm: npm_publisher_layer,
        });

        // done!

        TomlLayer {
            dist_version: cargo_dist_version,
            dist_url_override: cargo_dist_url_override,
            dist,
            allow_dirty,
            targets,
            artifacts: artifacts_layer,
            builds: build_layer,
            ci: ci_layer,
            hosts: host_layer,
            installers: installer_layer,
            publishers: publisher_layer,
        }
    }
}

fn list_to_bool_layer<I, T>(
    is_global: bool,
    list: &Option<Vec<I>>,
    item: I,
    val: impl FnOnce() -> Option<T>,
) -> Option<BoolOr<T>>
where
    I: Eq,
{
    // If this thing has values that need to be set, force the layer to exist
    if let Some(val) = val() {
        return Some(BoolOr::Val(val));
    };
    // If the list doesn't exist, don't mention it
    let list = list.as_ref()?;
    // Otherwise treat "is in the list" as a simple boolean
    let is_in_list = list.contains(&item);
    if is_global && !is_in_list {
        // ... with the exception of an omitted value in the global list.
        // here None and Some(false) are the same, so Some(false) is Noise
        // we want to hide.
        None
    } else {
        Some(BoolOr::Bool(is_in_list))
    }
}
