//! Logic for computing how different platforms are supported by a project's archives.
use axoproject::platforms::{
    TARGET_ARM64_MAC, TARGET_ARM64_WINDOWS, TARGET_X64_MAC, TARGET_X64_WINDOWS, TARGET_X86_WINDOWS,
};
use cargo_dist_schema::{ArtifactId, AssetId, DistManifest, Linkage, SystemInfo};
use serde::Serialize;

use crate::{
    backend::installer::{ExecutableZipFragment, UpdaterFragment},
    config::ZipStyle,
    BinaryKind, DistGraphBuilder, ReleaseIdx, SortedMap, TargetTriple,
};

/// Suffixes of TargetTriples that refer to statically linked linux libcs.
///
/// On Linux it's preferred to dynamically link libc *but* because the One True ABI
/// is actually the Linux kernel syscall interface, you *can* theoretically statically
/// link libc. This comes with various tradeoffs but the big selling point is that the
/// Linux kernel is a much more slowly moving target, so you can build a binary
/// that's portable across way more systems by statically linking libc. As such,
/// for any archive claiming to provide a static libc linux build, we can mark this
/// archive as providing support for any linux distro (for that architecture)
///
/// Currently rust takes "linux-musl" to mean "statically linked musl", but
/// in the future it will mean "dynamically linked musl":
///
/// https://github.com/rust-lang/compiler-team/issues/422
///
/// To avoid this ambiguity, we prefer "musl-static" and "musl-dynamic" aliases to
/// disambiguate this situation. This module immediately rename "musl" to "musl-static",
/// so in the following listings we don't need to deal with bare "musl".
///
/// Also note that known bonus ABI suffixes like "eabihf" are also already dealt with.
const LINUX_STATIC_LIBCS: &[&str] = &["linux-musl-static"];
/// Dynamically linked linux libcs that static libcs can replace
const LINUX_STATIC_REPLACEABLE_LIBCS: &[&str] = &["linux-gnu", "linux-musl-dynamic"];
/// A fake TargetTriple for apple's universal2 format (staples x64 and arm64 together)
const TARGET_MACOS_UNIVERSAL2: &str = "universal2-apple-darwin";

/// The quality of support an archive provides for a given platform
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SupportQuality {
    /// The archive natively supports this platform, there's no beating it
    HostNative,
    /// The archive natively supports this platform, but it's a Universal binary that contains
    /// multiple platforms stapled together, so if there are also more precise archives, prefer those.
    BulkyNative,
    /// The archive is still technically native to this platform, but it's in some sense
    /// imperfect. This can happen for things like "running a 32-bit binary on 64-bit" or
    /// "using a statically linked linux libc". This solution is acceptable, but a HostNative
    /// (or BulkyNative) solution should always be preferred.
    ImperfectNative,
    /// The archive is only running by the grace of pretty heavyweight emulation like Rosetta2.
    /// This should be treated as a last resort, but hey, it works!
    Emulated,
    /// The layers of emulation are out of control.
    Hellmulated,
    /// STOP
    HighwayToHellmulated,
}

/// A unixy libc version
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Serialize)]
pub struct LibcVersion {
    /// Major version
    major: u64,
    /// Series (minor) version
    series: u64,
}

/// Conditions that an installer should ideally check before using this an archive
#[derive(Debug, Clone, Default, Serialize)]
pub struct RuntimeConditions {
    /// The system glibc should be at least this version
    #[serde(skip_serializing_if = "Option::is_none")]
    min_glibc_version: Option<LibcVersion>,
    /// The system musl libc should be at least this version
    #[serde(skip_serializing_if = "Option::is_none")]
    min_musl_version: Option<LibcVersion>,
    /// Rosetta2 should be installed
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    rosetta2: bool,
}

/// Computed platform support details for a Release
#[derive(Debug, Clone, Default)]
pub struct PlatformSupport {
    /// The prebuilt archives for the Release
    pub archives: Vec<FetchableArchive>,
    /// The updaters for the Release
    pub updaters: Vec<FetchableUpdater>,
    /// Which options are available for the given target-triples.
    ///
    /// The list of PlatformEntries is pre-sorted in descending quality, so the first
    /// is the best and should be used if possible (but maybe there's troublesome RuntimeConditions).
    pub platforms: SortedMap<TargetTriple, Vec<PlatformEntry>>,
}

/// An archive of the prebuilt binaries for an app that can be fetched
#[derive(Debug, Clone)]
pub struct FetchableArchive {
    /// The unique id (and filename) of the archive
    pub id: ArtifactId,
    /// Runtime conditions that are native to this archive
    ///
    /// (You can largely ignore these in favour of the runtime_conditions in PlatformEntry)
    pub native_runtime_conditions: RuntimeConditions,
    /// What target triples does this archive natively support
    pub target_triples: Vec<TargetTriple>,
    /// The sha256sum of the archive
    pub sha256sum: Option<String>,
    /// The executables in the archive (may include .exe, assumed to be in root)
    pub executables: Vec<String>,
    /// The dynamic libraries in the archive (assumed to be in root)
    pub cdylibs: Vec<String>,
    /// The static libraries in the archive (assumed to be in root)
    pub cstaticlibs: Vec<String>,
    /// The kind of compression the archive has
    pub zip_style: ZipStyle,
    /// The updater you should also fetch if you install this archive
    pub updater: Option<FetchableUpdaterIdx>,
}

/// An updater for an app that can be fetched
#[derive(Debug, Clone)]
pub struct FetchableUpdater {
    /// The unique id (and filename) of the updater
    pub id: ArtifactId,
    /// The binary name of the updater
    pub binary: String,
}

/// An index into [`PlatformSupport::archives`][]
pub type FetchableArchiveIdx = usize;
/// An index into [`PlatformSupport::updaters`][]
pub type FetchableUpdaterIdx = usize;

/// An entry describing how well an archive supports a platform
#[derive(Debug, Clone)]
pub struct PlatformEntry {
    /// The quality of the support (prefer more "native" support over "emulated"/"fallback")
    pub quality: SupportQuality,
    /// Conditions the system being installed to must satisfy for the install to work.
    /// Ideally installers should check these before using this archive, and fall back to
    /// "worse" ones if the conditions aren't met.
    ///
    /// For instance if you have a linux-gnu build but the system glibc is too old to run it,
    /// you will want to skip it in favour of a more portable musl-static build.
    pub runtime_conditions: RuntimeConditions,
    /// The archive
    pub archive_idx: FetchableArchiveIdx,
}

impl PlatformSupport {
    /// Compute the PlatformSupport for a Release
    ///
    /// The later this information is computed, the richer it will be.
    /// For instance if this is (re)computed after builds, it will contain shasums.
    pub(crate) fn new(dist: &DistGraphBuilder, release_idx: ReleaseIdx) -> PlatformSupport {
        let mut platforms = SortedMap::<TargetTriple, Vec<PlatformEntry>>::new();
        let release = dist.release(release_idx);
        let mut archives = vec![];
        let mut updaters = vec![];
        // Gather up all the fetchable archives
        for &variant_idx in &release.variants {
            // Compute the updater this variant *would* make *if* it were built
            let updater_idx = if dist.inner.install_updater {
                let updater_artifact = dist.make_updater_for_variant(variant_idx);
                let updater = FetchableUpdater {
                    id: updater_artifact.id.clone(),
                    binary: updater_artifact.id.clone(),
                };
                let updater_idx = updaters.len();
                updaters.push(updater);
                Some(updater_idx)
            } else {
                None
            };

            // Compute the artifact zip this variant *would* make *if* it were built
            // FIXME: this is a kind of hacky workaround for the fact that we don't have a good
            // way to add artifacts to the graph and then say "ok but don't build it".
            let (artifact, binaries) =
                dist.make_executable_zip_for_variant(release_idx, variant_idx);

            let native_runtime_conditions =
                native_runtime_conditions_for_artifact(dist, &artifact.id);

            let executables = binaries
                .iter()
                .filter(|(idx, _)| dist.binary(*idx).kind == BinaryKind::Executable);
            let cdylibs = binaries
                .iter()
                .filter(|(idx, _)| dist.binary(*idx).kind == BinaryKind::DynamicLibrary);
            let cstaticlibs = binaries
                .iter()
                .filter(|(idx, _)| dist.binary(*idx).kind == BinaryKind::StaticLibrary);

            let archive = FetchableArchive {
                id: artifact.id,
                target_triples: artifact.target_triples,
                executables: executables
                    .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
                    .collect(),
                cdylibs: cdylibs
                    .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
                    .collect(),
                cstaticlibs: cstaticlibs
                    .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
                    .collect(),
                zip_style: artifact.archive.as_ref().unwrap().zip_style,
                sha256sum: None,
                native_runtime_conditions,
                updater: updater_idx,
            };
            archives.push(archive);
        }

        // Compute what platforms each archive Really supports
        for (archive_idx, archive) in archives.iter().enumerate() {
            let supports = supports(archive_idx, archive);
            for (target, support) in supports {
                platforms.entry(target).or_default().push(support);
            }
        }

        // Now sort the platform-support so the best options come first
        for support in platforms.values_mut() {
            support.sort_by(|a, b| {
                // Sort by SupportQuality, tie break by artifact name (for stability)
                a.quality.cmp(&b.quality).then_with(|| {
                    let archive_a = &archives[a.archive_idx];
                    let archive_b = &archives[b.archive_idx];
                    archive_a.id.cmp(&archive_b.id)
                })
            });
        }

        PlatformSupport {
            archives,
            updaters,
            platforms,
        }
    }

    /// Convert to the old-style format so we can gradually migrate
    pub fn fragments(&self) -> Vec<ExecutableZipFragment> {
        let mut fragments = vec![];
        for (target, options) in &self.platforms {
            let Some(option) = options.first() else {
                continue;
            };
            let archive = &self.archives[option.archive_idx];
            let updater = if let Some(updater_idx) = archive.updater {
                let updater = &self.updaters[updater_idx];
                Some(UpdaterFragment {
                    id: updater.id.clone(),
                    binary: updater.binary.clone(),
                })
            } else {
                None
            };
            let fragment = ExecutableZipFragment {
                id: archive.id.clone(),
                target_triple: target.clone(),
                zip_style: archive.zip_style,
                executables: archive.executables.clone(),
                cdylibs: archive.cdylibs.clone(),
                cstaticlibs: archive.cstaticlibs.clone(),
                runtime_conditions: option.runtime_conditions.clone(),
                updater,
            };
            fragments.push(fragment);
        }
        fragments
    }

    /// Conflate all the options that `fragments` suggests to create a single unified
    /// RuntimeConditions that can be used in installers while we transition to implementations
    /// that more granularly factor in these details.
    pub fn conflated_runtime_conditions(&self) -> RuntimeConditions {
        let mut runtime_conditions = RuntimeConditions::default();
        for (_target, options) in &self.platforms {
            let Some(option) = options.first() else {
                continue;
            };
            runtime_conditions.merge(&option.runtime_conditions);
        }
        runtime_conditions
    }
}

/// Given an archive, compute all the platforms it technically supports,
/// and to what level of quality.
///
/// It's fine to be very generous and repetitive here as long as SupportQuality
/// is honest and can be used to sort the options. Any "this is dubious" solutions
/// will be buried by more native/legit ones if they're available.
fn supports(
    archive_idx: FetchableArchiveIdx,
    archive: &FetchableArchive,
) -> Vec<(TargetTriple, PlatformEntry)> {
    let mut res = Vec::new();
    for target in &archive.target_triples {
        // For the following linux checks we want to pull off any "eabihf" suffix while
        // comparing/parsing libc types.
        let (degunked_target, abigunk) = if let Some(inner_target) = target.strip_suffix("eabihf") {
            (inner_target, "eabihf")
        } else {
            (target.as_str(), "")
        };

        // If this is the ambiguous-soon-to-be-changed "musl" target, rename it to musl-static,
        // which is its current behaviour.
        let (target, degunked_target) = if let Some(system) = degunked_target.strip_suffix("musl") {
            (
                format!("{system}musl-static{abigunk}"),
                format!("{degunked_target}-static"),
            )
        } else {
            (target.to_owned(), degunked_target.to_owned())
        };

        // First, add the target itself as a HostNative entry
        res.push((
            target.clone(),
            PlatformEntry {
                quality: SupportQuality::HostNative,
                runtime_conditions: archive.native_runtime_conditions.clone(),
                archive_idx,
            },
        ));

        // If this is a static linux libc, say it can support any linux at ImperfectNative quality
        for &static_libc in LINUX_STATIC_LIBCS {
            let Some(system) = degunked_target.strip_suffix(static_libc) else {
                continue;
            };
            for &libc in LINUX_STATIC_REPLACEABLE_LIBCS {
                res.push((
                    format!("{system}{libc}{abigunk}"),
                    PlatformEntry {
                        quality: SupportQuality::ImperfectNative,
                        runtime_conditions: archive.native_runtime_conditions.clone(),
                        archive_idx,
                    },
                ));
            }
            break;
        }

        // universal2 macos binaries are totally native for both arches, but bulkier than
        // necessary if we have builds for the individual platforms too.
        if target == TARGET_MACOS_UNIVERSAL2 {
            res.push((
                TARGET_X64_MAC.to_owned(),
                PlatformEntry {
                    quality: SupportQuality::BulkyNative,
                    runtime_conditions: archive.native_runtime_conditions.clone(),
                    archive_idx,
                },
            ));
            res.push((
                TARGET_ARM64_MAC.to_owned(),
                PlatformEntry {
                    quality: SupportQuality::BulkyNative,
                    runtime_conditions: archive.native_runtime_conditions.clone(),
                    archive_idx,
                },
            ));
        }

        // FIXME?: technically we could add "run 32-bit intel macos on 64-bit intel"
        // BUT this is unlikely to succeed as you increasingly need an EOL macOS,
        // as support was dropped in macOS Catalina (macOS 10.15, October 2019).
        // So this is unlikely to be helpful and DEFINITELY shouldn't be suggested
        // unless all installers enforce the check for OS version.

        // If this is x64 macos, say it can run on arm64 macos using Rosetta2
        // Note that Rosetta2 is not *actually* installed by default on Apple Silicon,
        // and the auto-installer for it only applies to GUI apps, not CLI apps, so ideally
        // any installer that uses this fallback should check if Rosetta2 is installed!
        if target == TARGET_X64_MAC {
            let runtime_conditions = RuntimeConditions {
                rosetta2: true,
                ..archive.native_runtime_conditions.clone()
            };
            res.push((
                TARGET_ARM64_MAC.to_owned(),
                PlatformEntry {
                    quality: SupportQuality::Emulated,
                    runtime_conditions,
                    archive_idx,
                },
            ));
        }

        // x86_32 windows binaries run fine on x86_64, but it's Imperfect compared to actual x86_64 binaries
        if target == TARGET_X86_WINDOWS {
            res.push((
                TARGET_X64_WINDOWS.to_owned(),
                PlatformEntry {
                    quality: SupportQuality::ImperfectNative,
                    runtime_conditions: archive.native_runtime_conditions.clone(),
                    archive_idx,
                },
            ));
        }

        // Windows' equivalent to Rosetta2 (CHPE) is in fact installed-by-default so no need to detect!
        if target == TARGET_X64_WINDOWS || target == TARGET_X86_WINDOWS {
            // prefer x64 over x86 if we have the option
            let quality = if target == TARGET_X86_WINDOWS {
                SupportQuality::Hellmulated
            } else {
                SupportQuality::Emulated
            };
            res.push((
                TARGET_ARM64_WINDOWS.to_owned(),
                PlatformEntry {
                    quality,
                    runtime_conditions: archive.native_runtime_conditions.clone(),
                    archive_idx,
                },
            ));
        }

        // windows-msvc binaries should always be acceptable on windows-gnu (mingw)
        //
        // FIXME: in theory x64-pc-windows-msvc and i686-pc-windows-msvc can run on
        // aarch64-pc-windows-gnu, as a hybrid of this rules and the CHPE rule above.
        // I don't want to think about computing the transitive closure of platform
        // support and how to do all the tie breaking ("HighwayToHellmulated"?), so
        // for now all 5 arm64 mingw users can be a little sad.
        if let Some(system) = target.strip_suffix("windows-msvc") {
            res.push((
                format!("{system}windows-gnu"),
                PlatformEntry {
                    quality: SupportQuality::ImperfectNative,
                    runtime_conditions: archive.native_runtime_conditions.clone(),
                    archive_idx,
                },
            ));
        }
    }
    res
}

impl RuntimeConditions {
    fn merge(&mut self, other: &Self) {
        let RuntimeConditions {
            min_glibc_version,
            min_musl_version,
            rosetta2,
        } = other;

        self.min_glibc_version =
            max_of_min_libc_versions(&self.min_glibc_version, min_glibc_version);
        self.min_musl_version = max_of_min_libc_versions(&self.min_musl_version, min_musl_version);
        self.rosetta2 |= rosetta2;
    }
}

/// Combine two min_libc_versions to get a new min that satisfies both
fn max_of_min_libc_versions(
    lhs: &Option<LibcVersion>,
    rhs: &Option<LibcVersion>,
) -> Option<LibcVersion> {
    match (*lhs, *rhs) {
        (None, None) => None,
        (Some(ver), None) | (None, Some(ver)) => Some(ver),
        (Some(lhs), Some(rhs)) => Some(lhs.max(rhs)),
    }
}

/// Compute the requirements for running the binaries of this release on its host platform
fn native_runtime_conditions_for_artifact(
    dist: &DistGraphBuilder,
    artifact_id: &ArtifactId,
) -> RuntimeConditions {
    let manifest = &dist.manifest;
    let Some(artifact) = manifest.artifacts.get(artifact_id) else {
        return RuntimeConditions::default();
    };

    let mut runtime_conditions = RuntimeConditions::default();
    for asset in &artifact.assets {
        let asset_conditions = native_runtime_conditions_for_asset(manifest, &asset.id);
        runtime_conditions.merge(&asset_conditions);
    }

    runtime_conditions
}

fn native_runtime_conditions_for_asset(
    manifest: &DistManifest,
    asset_id: &Option<AssetId>,
) -> RuntimeConditions {
    let Some(asset_id) = asset_id else {
        return RuntimeConditions::default();
    };
    let Some(asset) = &manifest.assets.get(asset_id) else {
        return RuntimeConditions::default();
    };
    let Some(linkage) = &asset.linkage else {
        return RuntimeConditions::default();
    };
    // This one's actually infallible but better safe than sorry...
    let Some(system) = manifest.systems.get(&asset.system) else {
        return RuntimeConditions::default();
    };

    // Get various libc versions
    let min_glibc_version = native_glibc_version(system, linkage);
    let min_musl_version = native_musl_version(system, linkage);

    // rosetta2 is never required to run a binary on its *host* platform
    let rosetta2 = false;
    RuntimeConditions {
        min_glibc_version,
        min_musl_version,
        rosetta2,
    }
}

/// Get the native glibc version this binary links against, to the best of our ability
fn native_glibc_version(system: &SystemInfo, linkage: &Linkage) -> Option<LibcVersion> {
    for lib in &linkage.system {
        // If this links against glibc, then we need to require that
        // TODO: make some check equivalent to this!
        if lib.is_glibc {
            // TODO: misty's branch has this
            if let Some(system_glibc) = system.glibc_version {
                // If there's a system libc, assume that's what it was built against
                return Some(system_glibc);
            } else {
                // If the system has no known libc version use Ubuntu 20.04's glibc as a guess
                return Some(LibcVersion {
                    major: 2,
                    series: 11,
                });
            }
        }
    }
    None
}

/// Get the native musl libc version this binary links against, to the best of our ability
fn native_musl_version(_system: &SystemInfo, _linkage: &Linkage) -> Option<LibcVersion> {
    // FIXME: this should be the same as glibc_version but we don't get this info yet!
    None
}
