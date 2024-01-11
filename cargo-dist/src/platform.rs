#![allow(missing_docs)]

use cargo_dist_schema::ArtifactId;

use crate::{ArtifactIdx, TargetTriple, SortedMap, DistGraph, Release, DistGraphBuilder, ReleaseIdx, backend::installer::ExecutableZipFragment};

const X64_MUSL_STATIC: &str = "x86_64-unknown-linux-musl-static";
const X64_MUSL_DYNAMIC: &str = "x86_64-unknown-linux-musl-dynamic";

pub enum SupportQuality {
    HostNative,
    ImperfectNative,
    Emulated,
}

pub enum RuntimeCondition {
    MinGlibcVersion { major: u64, series: u64 },
    MinMuslVersion  { major: u64, series: u64 },
    Rosetta2,
}

pub struct InitialPlatformSupport {
    pub platforms: SortedMap<TargetTriple, Vec<PlatformEntry>>,
}

pub struct FetchableArchive {
    pub id: ArtifactId,
    pub sha256sum: Option<String>,
    pub binaries: Vec<String>,
}

pub struct PlatformEntry {
    pub quality: SupportQuality,
}

struct FinalPlatformSupport {
    platforms: SortedMap<TargetTriple, Vec<FullPlatformEntry>>,
}

struct FullPlatformEntry {
}

impl InitialPlatformSupport {
    fn new(dist: &DistGraphBuilder, release_idx: ReleaseIdx) -> InitialPlatformSupport {
        let platforms = SortedMap::new();
        let release = dist.release(release_idx);
        for &variant_idx in &release.variants {
            let variant = dist.variant(variant_idx);
            // Compute the artifact zip this variant *would* make *if* it were built
            // FIXME: this is a kind of hacky workaround for the fact that we don't have a good
            // way to add artifacts to the graph and then say "ok but don't build it".
            let (artifact, binaries) = dist.make_executable_zip_for_variant(release_idx, variant_idx);
            let archive = ExecutableZipFragment {
                id: artifact.id,
                target_triples: artifact.target_triples,
                binaries: binaries
                .into_iter()
                .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
                .collect(),
                zip_style: artifact.archive.as_ref().unwrap().zip_style,
            };
        }

        InitialPlatformSupport {
            platforms,
        }
    }
}


/*
fn supports(platform: &str, conditions: &[RuntimeCondition]) -> Vec<&'static str, PlatformSupport> {
    let mut res = vec![(platform, conditions.to_owned(), ];
    if platform == X64_MUSL_STATIC {
        res.extend([
            (X64_MUSL_DYNAMIC, conditions.to_owned(),
        ]);
    } else if platform == X64_MUSL_DYNAMIC {
        res.extend([X64_MUSL_DYNAMIC]);
    }
}

fn add_support(platform: &'static str, conditions: impl IntoIterator<RuntimeCondition>, )

 */

/*

const X64_MACOS: &str = "x86_64-apple-darwin";
const ARM64_MACOS: &str = "aarch64-apple-darwin";
const X64_GNU: &str = "x86_64-unknown-linux-gnu";
const X64_MUSL: &str = "x86_64-unknown-linux-musl";

let mut has_x64_apple = false;
let mut has_arm_apple = false;
let mut has_gnu_linux = false;
let mut has_static_musl_linux = false;
// Currently always false, someday this build will exist
let has_dynamic_musl_linux = false;
for &variant_idx in &release.variants {
    let variant = self.variant(variant_idx);
    let target = &variant.target;
    if target == X64_MACOS {
        has_x64_apple = true;
    }
    if target == ARM64_MACOS {
        has_arm_apple = true;
    }
    if target == X64_GNU {
        has_gnu_linux = true;
    }
    if target == X64_MUSL {
        has_static_musl_linux = true;
    }
}
let do_rosetta_fallback = has_x64_apple && !has_arm_apple;
let do_gnu_to_musl_fallback = !has_gnu_linux && has_static_musl_linux;
let do_musl_to_musl_fallback = has_static_musl_linux && !has_dynamic_musl_linux;

// Gather up the bundles the installer supports
let mut artifacts = vec![];
let mut target_triples = SortedSet::new();
for &variant_idx in &release.variants {
    let variant = self.variant(variant_idx);
    let target = &variant.target;
    if target.contains("windows") {
        continue;
    }
    // Compute the artifact zip this variant *would* make *if* it were built
    // FIXME: this is a kind of hacky workaround for the fact that we don't have a good
    // way to add artifacts to the graph and then say "ok but don't build it".
    let (artifact, binaries) =
        self.make_executable_zip_for_variant(to_release, variant_idx);
    target_triples.insert(target.clone());
    let mut fragment = ExecutableZipFragment {
        id: artifact.id,
        target_triples: artifact.target_triples,
        zip_style: artifact.archive.as_ref().unwrap().zip_style,
        binaries: binaries
            .into_iter()
            .map(|(_, dest_path)| dest_path.file_name().unwrap().to_owned())
            .collect(),
    };
    if do_rosetta_fallback && target == X64_MACOS {
        // Copy the info but respecify it to be arm64 macos
        let mut arm_fragment = fragment.clone();
        arm_fragment.target_triples = vec![ARM64_MACOS.to_owned()];
        artifacts.push(arm_fragment);
    }
    if target == X64_MUSL {
        // musl-static is actually kind of a fake triple we've invented
        // to let us specify which is which; we want to ensure it exists
        // for the installer to act on
        fragment.target_triples = vec![X64_MUSL_STATIC.to_owned()];
    }
    if do_gnu_to_musl_fallback && target == X64_MUSL {
        // Copy the info but lie that it's actually glibc
        let mut musl_fragment = fragment.clone();
        musl_fragment.target_triples = vec![X64_GNU.to_owned()];
        artifacts.push(musl_fragment);
    }
    if do_musl_to_musl_fallback && target == X64_MUSL {
        // Copy the info but lie that it's actually dynamic musl
        let mut musl_fragment = fragment.clone();
        musl_fragment.target_triples = vec![X64_MUSL_DYNAMIC.to_owned()];
        artifacts.push(musl_fragment);
    }

    artifacts.push(fragment);
}

*/