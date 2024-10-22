//! Logic for computing how different platforms are supported by a project's archives.
//!
//! The main entrypoint for this is [`PlatformSupport::new`][].
//! [`PlatformSupport::platforms`][] is what you want to query.
//!
//!
//! # Platform Support
//!
//! The complexity of this module is trying to handle things like:
//!
//! * linux-musl-static binaries work on linux-gnu platforms
//! * but linux-gnu binaries are preferrable if they're available
//! * but if the target system has a really old version of glibc, the linux-gnu binaries won't work
//! * so the linux-musl-static binaries need to still be eligible as a fallback
//!
//! ("x64 macos binaries can run on arm64 macos under rosetta2" is another good canonical example)
//!
//! [`PlatformSupport::platforms`][] is an index
//! from "target I want to install to" ([`TargetTriple`][])
//! to "list of archives we can potentially use to do that" ([`PlatformEntry`][]).
//! The list is sorted in decreasing order from best-to-worst options. The basic idea
//! is that you go down that list and try each option in order until one "works".
//! Typically there will only be one option, and that option will always work.
//!
//!
//!
//! ## SupportQuality
//!
//! We get *multiple* options when there are targets that interop, typically
//! with an emulation layer or some kind of compromise. The level of compromise
//! is captured by [`SupportQuality`][], which is what we sort the options by.
//! It can be found on [`PlatformEntry::quality`][].
//!
//! For instance, on linux-gnu, linux-gnu binaries have [`SupportQuality::HostNative`][] (best)
//! while linux-musl-static binaris have [`SupportQuality::ImperfectNative`][] (excellent).
//!
//! Note that this `SupportQuality` is specific to the target platform. For instance
//! x64 macos binaries are [`SupportQuality::HostNative`][] on x86_64-apple-darwin but
//! [`SupportQuality::Emulated`][] on aarch64-apple-darwin (they run via Rosetta 2).
//!
//!
//! ## RuntimeConditions
//!
//! A technically-superior option can *fail* if there are known runtime conditions for
//! it to execute properly on the install-target system, and the system doesn't satisfy
//! those conditions. These conditions are captured by [`RuntimeConditions`][].
//! It can be found on [`PlatformEntry::runtime_conditions`][].
//!
//! For instance, linux-gnu binaries are built against a specific version of glibc.
//! It can work with any glibc *newer* than that version, but it will hard error out
//! with a glibc *older* than that version.
//!
//! It's up to each installer to check these conditions to the best of their ability
//! and discard options that won't work. As of this writing, the shell installer
//! does the best job of this, because linux has the most relevant fallback/conditions.
//!
//!
//! ## Native RuntimeConditions
//!
//! Note that [`FetchableArchive::native_runtime_conditions`][] also exists but
//! **YOU PROBABLY DON'T WANT THAT VALUE**. It contains runtime conditions that
//! are *intrinsic* to the archive, which is a subset of [`PlatformEntry::runtime_conditions`][].
//!
//! For instance the glibc version is intrinsic to a linux-gnu archive, and
//! is therefore a native_runtime_condition, so it will show up in both places.
//! However "must have Rosetta2 installed" isn't intrinsic to x64 macos binaries,
//! it *only* applies to "x64 macos binaries on arm64 macos", and so will *only*
//! appear in [`PlatformEntry::runtime_conditions`][].
//!
//!
//! # When To Invoke This Subsystem
//!
//! [`PlatformSupport::new`][] can be called at any time, and will do its best to produce
//! the best possible results with the information it has. However, the later
//! this function can be (re)run, the better information it will have.
//!
//! In particular, only once we have info from building and linkage-checking
//! the binaries will we have all the [`RuntimeConditions`][]. In a typical
//! CI run of cargo-dist this is fine, because the main use of this info
//! is for installers, which are built with a fresh invocation on a machine
//! with all binaries/platform info prefetched.
//!
//! However, if you were to run cargo-dist locally and try to build binaries
//! and installers all at once, we currently fail to regenerate the platform
//! info and update the installers. Doing this would necessitate some refactors
//! to make the installers compute more of their archive/platform info "latebound"
//! instead of the current very eager approach where we do that when building the
//! DistGraph.
//!
//! In an ideal world we do an initial invocation of this API when building the DistGraph
//! to get the list of platforms we expect to support (to know what an installer depends on),
//! and then after building all binarier/archives and running linkage, we would rerun
//! this API to get the final/complete picture. Then when we go to build installers we
//! would lookup the *details* of PlatformSupport.
//!
//!
//! # Compatibility Shims
//!
//! There's lots of things that care about platforms/archives, and they were written
//! before this module. As of this writing we're in the process of gradually migrating
//! them to using the full power of this API.
//!
//! To enable that migration, the PlatformSupport has a few APIs that will squash its
//! richer information into legacy/simpler ones. In an ideal world we stop using these
//! APIs and migrate all installers to just Doing It Right (but Doing It Right
//! moves more logic into each installer, as it essentially requires each installer
//! to have a full implementation for how to query [`PlatformSupport::platforms`][]
//! and do the RuntimeCondition fallbacks.
//!
//!
//! ## Fragments
//!
//! Fragments is the old platform support format that this API was made to replace.
//!
//! [`PlatformSupport::fragments`][] throws out all the fallback/condition information
//! to produce a list of archives, each with a single target it claims to support.
//! In cases where e.g. you have linux-musl-static build but no linux-gnu build, we
//! will emit multiple copies of the linux-musl-static archive, one for each platform
//! it's the best option for (so typically 3 copies covering linux-musl-static,
//! linux-musl-dynamic, and linux-gnu).
//!
//! This system is a lot easier for an installer to handle, because all it needs to
//! do is compute the target-triple it wants to try to install, and get the one
//! archive that claims to support that (or error if none).
//!
//! Historically things like musl fallback were implemented in an installer during
//! its target-triple selection with a single global hardcoded glibc version.
//!
//!
//! ## Conflated Runtime Conditions
//!
//! [`PlatformSupport::safe_conflated_runtime_conditions`][] and
//! [`PlatformSupport::conflated_runtime_conditions`][] exist to deal with
//! installers that have the above "single global hardcoded glibc version"
//! mentioned in the previous section.
//!
//! It represents a half-step to removing that, by removing the "hardcoded"
//! part, having the version be baked into the installer when we generate it.
//!
//! The "conflation" occurs when you have multiple linux-gnu platforms.
//! This is typical if you build for x64 and arm64 linux. In this case, the
//! runners may have different glibc versions, so there's no "correct"
//! global hardcoded version.
//!
//! Conflated conditions handle this by taking the maximum, which is *safe*
//! but may prevent people from installing on a compatible system (see
//! the next section for details).
//!
//!
//! # The Importance of Glibc Versions
//!
//! Getting the right glibc version is important because it's used to:
//!
//! * Trigger musl fallback in installers if your glibc is too new
//! * Informatively error out installers if there is no musl fallback
//!
//! If the version is wrong, there are two kinds of failure mode.
//!
//! If this version is too new, we may spuriously error during install for overly
//! strict constraints, preventing users from installing the application at all.
//! If there is a musl-static fallback this isn't a concern, and instead we'll just
//! overly-aggressively use the musl fallback (though it's mildly unfortunate that
//! a "more native" option is available and unused).
//!
//! If this version is too old we will fail to error and/or fail to invoke musl fallback,
//! and may claim to succesfully install linux-gnu binaries which will immediately error out
//! when run.
//!
//!
//! ### Madeup Glibc Versions
//!
//! There is currently a FIXME in `native_runtime_conditions_for_artifact` about us making up a fake
//! glibc version if we can't find one, but we're clearing supposed to be linking linux-gnu.
//!
//! Under ideal conditions this only is "transiently" used when we're too-eagerly looking up
//! runtime conditions, or doing tests without linkage info. As such, they
//! generally won't appear in final production installers.
//! In this case they will get an "arbitrary" glibc version ([`LibcVersion::default_glibc`][]).
//!
//! *HOWEVER* there are genuine situations where we don't run linkage in production.
//! For instance, if the archives were built and packaged in custom build
//! steps, because the user wanted to use maturin for cross-compilation.
//!
//!
//! ### Approximating Glibc Versions
//!
//! To the best of our knowledge, there is no way to "ask" a binary what version of glibc
//! it's linked against (if this is wrong PLEASE let us know that would be so useful).
//! It will tell you it's linked against glibc, but not the version
//! (there's a version in the library name but that never changes and is therefore irrelevant).
//!
//! We approximate the answer by asking the glibc on the system that built the binary
//! "hey what version are you" and then *ASSUME ALL BINARIES BUILT ON THAT PLATFORM WERE
//! BUILT AGAINST IT*.
//!
//! This is the default for most toolchains and is correct in 99% of cases.
//! However, some tools may go above and beyond to try to link against older glibcs.
//! Tools such as maturin and zig do this. In this case we are likely to pick a too-new
//! glibc version, see the previous sections for the implications of this.
//! It's possible in the case of maturin you "just" need to check the glibc in the
//! docker image it used? This is a guess though.
//!
//!
//! # targets vs target
//!
//! Ok so a lot of cargo-dist's code is *vaguely* trying to allow for a single archive
//! to *natively* be built for multiple architectures. This would for instance be the
//! case for any apple Universal Binary, which is just several binaries built for different
//! architectures all stapled together.
//!
//! This is why you'll see several places where an archive/binary has `targets`, *plural*.
//!
//! In practice this is headache inducing, and because nothing we support *actually*
//! is like this, code variously has punted on supporting it, or asserts against it.
//! As such, there's a lot of random places where we use `target`, *singular*.
//! Typically `target` is just `targets[0]`.
//!
//! So anyway it would be cool if code tried to work with `targets` but if you see stuff
//! only using target, or weirdly throwing out parts of targets... that's why.
//!
//! In theory *this* is the module that would handle it for everyone else, because once
//! we've constructed [`PlatformSupport`][] the information is indexed such that the
//! difference doesn't actually matter (nothing should care what platform an archive
//! is *natively* for, they should just do whatever [`PlatformSupport::platforms`][] says).
//!
//! But until we care about universal binaries, it's not really worth dealing with.

#![allow(rustdoc::private_intra_doc_links)]

use cargo_dist_schema::{
    ArtifactId, AssetId, BuildEnvironment, DistManifest, GlibcVersion, Linkage, SystemInfo,
    TargetTriple,
};
use serde::Serialize;

use crate::{
    backend::installer::{ExecutableZipFragment, UpdaterFragment},
    config::ZipStyle,
    platforms::{
        TARGET_ARM64_MAC, TARGET_ARM64_WINDOWS, TARGET_X64_MAC, TARGET_X64_WINDOWS,
        TARGET_X86_WINDOWS,
    },
    BinaryKind, DistGraphBuilder, ReleaseIdx, SortedMap,
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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
    pub major: u64,
    /// Series (minor) version
    pub series: u64,
}

impl LibcVersion {
    /// Get the default glibc version for cases where we just need to guess
    /// and make one up.
    ///
    /// This is the glibc of Ubuntu 20.04, which is the oldest supported
    /// github linux runner, as of this writing.
    pub fn default_glibc() -> Self {
        Self {
            major: 2,
            series: 31,
        }
    }

    fn glibc_from_schema(schema: &GlibcVersion) -> Self {
        Self {
            major: schema.major,
            series: schema.series,
        }
    }
}

/// Conditions that an installer should ideally check before using this an archive
#[derive(Debug, Clone, Default, Serialize)]
pub struct RuntimeConditions {
    /// The system glibc should be at least this version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_glibc_version: Option<LibcVersion>,
    /// The system musl libc should be at least this version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_musl_version: Option<LibcVersion>,
    /// Rosetta2 should be installed
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rosetta2: bool,
}

/// Computed platform support details for a Release
#[derive(Debug, Clone, Default, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct FetchableArchive {
    /// The unique id (and filename) of the archive
    pub id: ArtifactId,
    /// Runtime conditions that are native to this archive
    ///
    /// (You can largely ignore these in favour of the runtime_conditions in PlatformEntry)
    pub native_runtime_conditions: RuntimeConditions,
    /// "The" target triple to use
    pub target_triple: TargetTriple,
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
            let updater_idx = if dist.inner.config.installers.updater {
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
                // computed later
                target_triple: TargetTriple::new("".to_owned()),
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
        for (archive_idx, archive) in archives.iter_mut().enumerate() {
            let supports = supports(archive_idx, archive);
            // FIXME: some places need us to pick a simple single target triple
            // and it needs to have desugarrings that `supports` computes, so we
            // just grab the first triple, which is always going to be a native one
            if let Some((target, _)) = supports.first() {
                archive.target_triple.clone_from(target);
            }
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
        for options in self.platforms.values() {
            let Some(option) = options.first() else {
                continue;
            };
            runtime_conditions.merge(&option.runtime_conditions);
        }
        runtime_conditions
    }

    /// Similar to conflated_runtime_conditions, but certain None values
    /// are replaced by safe defaults.
    /// Currently, a default value is provided for glibc; others may be
    /// provided in the future.
    pub fn safe_conflated_runtime_conditions(&self) -> RuntimeConditions {
        let mut runtime_conditions = self.conflated_runtime_conditions();
        if runtime_conditions.min_glibc_version.is_none() {
            runtime_conditions.min_glibc_version = Some(LibcVersion::default_glibc());
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
    let mut res: Vec<(TargetTriple, PlatformEntry)> = Vec::new();
    for target in &archive.target_triples {
        // this whole function manipulates targets as a string slice, which
        // is unfortunate — these manipulations would be better done on a
        // "parsed" version of the target
        let target = target.as_str();

        // For the following linux checks we want to pull off any "eabihf" suffix while
        // comparing/parsing libc types.
        let (degunked_target, abigunk) = if let Some(inner_target) = target.strip_suffix("eabihf") {
            (inner_target, "eabihf")
        } else {
            (target, "")
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
            TargetTriple::new(target.clone()),
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
                    TargetTriple::new(format!("{system}{libc}{abigunk}")),
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

        let target = TargetTriple::new(target);

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
        if let Some(system) = target.as_str().strip_suffix("windows-msvc") {
            res.push((
                TargetTriple::new(format!("{system}windows-gnu")),
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
    let mut runtime_conditions = RuntimeConditions::default();
    if let Some(artifact) = manifest.artifacts.get(artifact_id) {
        for asset in &artifact.assets {
            let asset_conditions = native_runtime_conditions_for_asset(manifest, &asset.id);
            runtime_conditions.merge(&asset_conditions);
        }
    };
    // FIXME: in our test suite we're running bare artifacts=global so we're missing
    // all artifact/linkage info, preventing basic glibc bounds
    if artifact_id.contains("linux")
        && artifact_id.contains("-gnu")
        && runtime_conditions.min_glibc_version.is_none()
    {
        runtime_conditions.min_glibc_version = Some(LibcVersion::default_glibc());
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
        if lib.is_glibc() {
            if let BuildEnvironment::Linux {
                glibc_version: Some(system_glibc),
            } = &system.build_environment
            {
                // If there's a system libc, assume that's what it was built against
                return Some(LibcVersion::glibc_from_schema(system_glibc));
            } else {
                // If the system has no known libc version use Ubuntu 20.04's glibc as a guess
                return Some(LibcVersion::default_glibc());
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
