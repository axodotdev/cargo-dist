//! Code for generating npm-installer.tar.gz

use axoasset::{LocalAsset, SourceFile};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_dist_schema::{GlibcVersion, TargetTriple};
use serde::Serialize;

use super::InstallerInfo;
use crate::{
    backend::templates::{Templates, TEMPLATE_INSTALLER_NPM, TEMPLATE_INSTALLER_NPM_RUN_JS},
    errors::DistResult,
    platform::LibcVersion,
    DistGraph, SortedMap, SortedSet,
};

/// Info about an npm installer
#[derive(Debug, Clone, Serialize)]
pub struct NpmInstallerInfo {
    /// The name of the npm package
    pub npm_package_name: String,
    /// The version of the npm package
    pub npm_package_version: String,
    /// Short description of the package
    pub npm_package_desc: Option<String>,
    /// URL to repository
    pub npm_package_repository_url: Option<String>,
    /// URL to homepage
    pub npm_package_homepage_url: Option<String>,
    /// Short description of the package
    pub npm_package_authors: Vec<String>,
    /// Short description of the package
    pub npm_package_license: Option<String>,
    /// Array of keywords for this package
    pub npm_package_keywords: Option<Vec<String>>,
    /// Dir to build the package in
    pub package_dir: Utf8PathBuf,
    /// Generic installer info
    pub inner: InstallerInfo,
}

const RUN_JS: &str = "run.js";
const PACKAGE_JSON: &str = "package.json";
const PACKAGE_LOCK: &str = "npm-shrinkwrap.json";

type PackageJsonPlatforms = SortedMap<TargetTriple, PackageJsonPlatform>;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackageJsonPlatform {
    artifact_name: String,
    bins: SortedMap<String, String>,
    zip_ext: String,
}

#[derive(Debug, Clone, Default)]
struct PlatformSummary {
    bins: SortedSet<String>,
    aliases: SortedMap<String, String>,
    platforms: PackageJsonPlatforms,
}

#[derive(Serialize, Debug, Clone)]
struct RunInfo {
    bin: String,
}

pub(crate) fn write_npm_project(dist: &DistGraph, info: &NpmInstallerInfo) -> DistResult<()> {
    // First render the dir
    let templates = &dist.templates;
    let mut files = templates.render_dir_to_clean_strings(TEMPLATE_INSTALLER_NPM, info)?;
    let platforms = platforms(info);
    mangle_run_js(templates, &platforms, &mut files)?;
    mangle_package_json(info, &platforms, &mut files)?;
    mangle_package_lock(info, &platforms, &mut files)?;

    // Finally, write the results
    let zip_dir = &info.package_dir;
    for (relpath, rendered) in files {
        LocalAsset::write_new_all(&rendered, zip_dir.join(relpath))?;
    }

    Ok(())
}

fn mangle_run_js(
    templates: &Templates,
    platforms: &PlatformSummary,
    files: &mut SortedMap<Utf8PathBuf, String>,
) -> DistResult<()> {
    // There's a run.js.j2 that we actually want to render once-per-binary, so remove this copy
    let run_js_path = Utf8Path::new(RUN_JS);
    files
        .remove(run_js_path)
        .expect("npm template didn't have a run.js!?");

    for bin in &platforms.bins {
        let filename = platforms.run_js_name_for_bin(bin);
        let info = RunInfo {
            bin: bin.to_owned(),
        };
        let contents =
            templates.render_file_to_clean_string(TEMPLATE_INSTALLER_NPM_RUN_JS, &info)?;
        files.insert(Utf8PathBuf::from(filename), contents);
    }

    Ok(())
}

fn mangle_package_lock(
    info: &NpmInstallerInfo,
    platforms: &PlatformSummary,
    files: &mut SortedMap<Utf8PathBuf, String>,
) -> DistResult<()> {
    let package_lock_path = Utf8Path::new(PACKAGE_LOCK);
    // Now mangle the package.json with data we want
    let orig_package_lock = files
        .remove(package_lock_path)
        .expect("npm template didn't have package.json!?");
    let package_lock_src = SourceFile::new(PACKAGE_LOCK, orig_package_lock);
    let mut package_lock = package_lock_src.deserialize_json::<serde_json::Value>()?;

    // top-level details
    package_lock["name"] = info.npm_package_name.clone().into();
    package_lock["version"] = info.npm_package_version.clone().into();
    // info for inner root package
    // Yes, this is genuinely keyed by the empty string in the actual npm-shrinkwrap.json format.
    let root_package = &mut package_lock["packages"][""];
    root_package["name"] = info.npm_package_name.clone().into();
    root_package["version"] = info.npm_package_version.clone().into();
    if let Some(val) = info.npm_package_license.clone() {
        root_package["license"] = val.into();
    }
    // installer-specific fields
    root_package["bin"] = platforms.bins_json();

    // Commit the new package.json
    let new_package_lock = serde_json::to_string_pretty(&package_lock).expect("serde_json failed");
    files.insert(package_lock_path.to_owned(), new_package_lock);

    Ok(())
}

fn mangle_package_json(
    info: &NpmInstallerInfo,
    platforms: &PlatformSummary,
    files: &mut SortedMap<Utf8PathBuf, String>,
) -> DistResult<()> {
    let package_json_path = Utf8Path::new(PACKAGE_JSON);
    // Now mangle the package.json with data we want
    let orig_package_json = files
        .remove(package_json_path)
        .expect("npm template didn't have package.json!?");
    let package_json_src = SourceFile::new(PACKAGE_JSON, orig_package_json);
    let mut package_json = package_json_src.deserialize_json::<serde_json::Value>()?;

    // Basic metadata
    package_json["name"] = info.npm_package_name.clone().into();
    package_json["version"] = info.npm_package_version.clone().into();
    if let Some(val) = info.npm_package_desc.clone() {
        package_json["description"] = val.into();
    }
    if let Some(val) = info.npm_package_repository_url.clone() {
        package_json["repository"] = val.into();
    }
    if let Some(val) = info.npm_package_homepage_url.clone() {
        package_json["homepage"] = val.into();
    }
    if let Some(val) = info.npm_package_license.clone() {
        package_json["license"] = val.into();
    }
    if let Some(val) = info.npm_package_keywords.clone() {
        package_json["keywords"] = val.into();
    }
    if info.npm_package_authors.len() > 1 {
        package_json["contributors"] = info.npm_package_authors.clone().into();
    } else if !info.npm_package_authors.is_empty() {
        package_json["author"] = info.npm_package_authors[0].clone().into();
    }
    // installer-specific fields
    package_json["bin"] = platforms.bins_json();
    // These two fields are "non-standard" in the package.json format, but the
    // installer expects to find them when it reads its own package.json (with `require`).
    // It's fairly normal to add random stuff to a package.json like this,
    // as it's a format that's infamously ill-defined with minimal validation.
    package_json["artifactDownloadUrl"] = info.inner.base_url.clone().into();
    package_json["supportedPlatforms"] = platforms.platform_support_json();

    match info.inner.runtime_conditions.min_glibc_version {
        Some(LibcVersion { major, series }) => {
            package_json["glibcMinimum"] = glibc_json(major, series)
        }
        _ => {
            let default = GlibcVersion::default();
            package_json["glibcMinimum"] = glibc_json(default.major, default.series)
        }
    }

    // Commit the new package.json
    let new_package_json = serde_json::to_string_pretty(&package_json).expect("serde_json failed");
    files.insert(package_json_path.to_owned(), new_package_json);

    Ok(())
}

fn glibc_json(major: u64, series: u64) -> serde_json::Value {
    let mut map = SortedMap::<&str, u64>::new();
    map.insert("major", major);
    map.insert("series", series);

    serde_json::to_value(&map).expect("serde_json failed")
}

fn platforms(info: &NpmInstallerInfo) -> PlatformSummary {
    let mut output = PlatformSummary::default();
    for archive in &info.inner.artifacts {
        let target = archive.target_triple.clone();

        let mut bins = SortedMap::new();
        for bin in &archive.executables {
            // Add the binary
            let raw_name = bin.strip_suffix(".exe").unwrap_or(bin);
            bins.insert(raw_name.to_owned(), bin.to_owned());
            output.bins.insert(raw_name.to_owned());

            // Add any aliases for this binary
            // (Aliases need to be statically declared in npm, so this code is essentially
            // taking the union of all aliases across all platforms, which in the current
            // impl will get the same result as doing things more precisely).
            let Some(alias_map) = info.inner.bin_aliases.get(&target) else {
                continue;
            };
            let Some(aliases) = alias_map.get(bin) else {
                continue;
            };
            for alias in aliases {
                let raw_alias_name = alias.strip_suffix(".exe").unwrap_or(alias);
                output
                    .aliases
                    .insert(raw_alias_name.to_owned(), raw_name.to_owned());
            }
        }

        let platform = PackageJsonPlatform {
            artifact_name: archive.id.clone(),
            bins,
            zip_ext: archive.zip_style.ext().to_owned(),
        };
        output.platforms.insert(target, platform);
    }
    output
}

impl PlatformSummary {
    fn bins_json(&self) -> serde_json::Value {
        let mut bins = SortedMap::<String, String>::new();
        for bin in &self.bins {
            let path = self.run_js_name_for_bin(bin);
            bins.insert(bin.to_owned(), path);
        }
        for (alias, bin) in &self.aliases {
            let path = self.run_js_name_for_bin(bin);
            bins.insert(alias.to_owned(), path);
        }
        serde_json::to_value(&bins).expect("serde_json failed")
    }
    fn platform_support_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.platforms).expect("serde_json failed")
    }
    fn run_js_name_for_bin(&self, bin: &str) -> String {
        format!("run-{bin}.js")
    }
}
