#![allow(missing_docs)]

use axoproject::platforms::{
    TARGET_ARM64_MAC, TARGET_ARM64_WINDOWS, TARGET_X64_MAC, TARGET_X64_WINDOWS, TARGET_X86_MAC,
    TARGET_X86_WINDOWS,
};
use cargo_dist_schema::ArtifactId;

use crate::{config::ZipStyle, DistGraphBuilder, ReleaseIdx, SortedMap, TargetTriple};

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
/// Currently rust takes "linux-musl" to mean "statically linked musl"a
/// in the future it will mean "dynamically linked musl":
///
/// https://github.com/rust-lang/compiler-team/issues/422
///
/// We prefer "musl-static" and "musl-dynamic" aliases to disambiguate this situation,
/// but support bare musl with the current semantics:
///
/// FIXME: when rustc makes the change, move "linux-musl" from STATIC_LIBCS to REPLACEABLE_LIBCS!
/// (this may involve detecting the current rust toolchain).
const LINUX_STATIC_LIBCS: &[&str] = &["linux-musl-static", "linux-musl"];
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCondition {
    MinGlibcVersion { major: u64, series: u64 },
    MinMuslVersion { major: u64, series: u64 },
    Rosetta2,
}

#[derive(Debug, Clone, Default)]
pub struct PlatformSupport {
    pub archives: Vec<FetchableArchive>,
    pub platforms: SortedMap<TargetTriple, Vec<PlatformEntry>>,
}

#[derive(Debug, Clone)]
pub struct FetchableArchive {
    pub id: ArtifactId,
    pub native_runtime_conditions: Vec<RuntimeCondition>,
    pub target_triples: Vec<TargetTriple>,
    pub sha256sum: Option<String>,
    pub binaries: Vec<String>,
    pub zip_style: ZipStyle,
}

pub type FetchableArchiveIdx = usize;

#[derive(Debug, Clone)]
pub struct PlatformEntry {
    pub quality: SupportQuality,
    pub runtime_conditions: Vec<RuntimeCondition>,
    pub archive_idx: FetchableArchiveIdx,
}

impl PlatformSupport {
    pub(crate) fn new(dist: &DistGraphBuilder, release_idx: ReleaseIdx) -> PlatformSupport {
        let mut platforms = SortedMap::<TargetTriple, Vec<PlatformEntry>>::new();
        let release = dist.release(release_idx);
        let mut archives = vec![];

        // Gather up all the fetchable archives
        for &variant_idx in &release.variants {
            // Compute the artifact zip this variant *would* make *if* it were built
            // FIXME: this is a kind of hacky workaround for the fact that we don't have a good
            // way to add artifacts to the graph and then say "ok but don't build it".
            let (artifact, binaries) =
                dist.make_executable_zip_for_variant(release_idx, variant_idx);
            let archive = FetchableArchive {
                id: artifact.id,
                target_triples: artifact.target_triples,
                binaries: binaries
                    .into_iter()
                    .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
                    .collect(),
                zip_style: artifact.archive.as_ref().unwrap().zip_style,
                sha256sum: None,
                native_runtime_conditions: vec![],
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
        for (_platform, support) in &mut platforms {
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
            platforms,
        }
    }
}

fn supports(
    archive_idx: FetchableArchiveIdx,
    archive: &FetchableArchive,
) -> Vec<(TargetTriple, PlatformEntry)> {
    let mut res = Vec::new();
    for target in &archive.target_triples {
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
            if let Some(system) = target.strip_suffix(static_libc) {
                for &libc in LINUX_STATIC_REPLACEABLE_LIBCS {
                    res.push((
                        format!("{system}{libc}"),
                        PlatformEntry {
                            quality: SupportQuality::ImperfectNative,
                            runtime_conditions: archive.native_runtime_conditions.clone(),
                            archive_idx,
                        },
                    ));
                }
                break;
            }
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

        // x86_32 macos binaries ran fine on x86_64, but it's Imperfect compared to actual x86_64 binaries
        // ...UP UNTIL macOS Catalina (macOS 10.15, October 2019)!
        //
        // FIXME: add some condition for "check the macos version" if we care about this!
        if target == TARGET_X86_MAC {
            res.push((
                TARGET_X64_MAC.to_owned(),
                PlatformEntry {
                    quality: SupportQuality::ImperfectNative,
                    runtime_conditions: archive.native_runtime_conditions.clone(),
                    archive_idx,
                },
            ));
        }

        // If this is x64 macos, say it can support arm64 macos using Rosetta2
        // Note that Rosetta2 is not *actually* installed by default on Apple Silicon,
        // and the auto-installer for it only applies to GUI apps, not CLI apps, so ideally
        // any installer that uses this fallback should check if Rosetta2 is installed!
        if target == TARGET_X64_MAC {
            let runtime_conditions = archive
                .native_runtime_conditions
                .iter()
                .cloned()
                .chain(Some(RuntimeCondition::Rosetta2))
                .collect();
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
        //
        // FIXME: ideally if both 32-bit and 64-bit binaries exist, the 64-bit should presumably be preferred?
        // do we want a "SupportQuality::Hellmulated" for i686-on-arm64?
        if target == TARGET_X64_WINDOWS || target == TARGET_X86_WINDOWS {
            res.push((
                TARGET_ARM64_WINDOWS.to_owned(),
                PlatformEntry {
                    quality: SupportQuality::Emulated,
                    runtime_conditions: archive.native_runtime_conditions.clone(),
                    archive_idx,
                },
            ));
        }
    }
    res
}
