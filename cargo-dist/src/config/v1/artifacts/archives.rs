//! archive config

use super::*;

/// archive config (final)
#[derive(Debug, Clone)]
pub struct ArchiveConfig {
    /// Include the following static files in bundles like archives.
    pub include: Vec<Utf8PathBuf>,
    /// Whether to auto-include files like `README*`, `(UN)LICENSE*`, `RELEASES*`, and `CHANGELOG*`
    pub auto_includes: bool,
    /// The archive format to use for windows builds (defaults .zip)
    pub windows_archive: ZipStyle,
    /// The archive format to use for non-windows builds (defaults .tar.xz)
    pub unix_archive: ZipStyle,
    /// Whether to include built libraries in the release archive
    pub package_libraries: Vec<LibraryStyle>,
    /// Binaries for a given platform
    pub binaries: SortedMap<String, Vec<String>>,
}

/// archive config (raw from config file)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ArchiveLayer {
    /// Include the following static files in bundles like archives.
    ///
    /// Paths are relative to the Cargo.toml this is defined in.
    ///
    /// Files like `README*`, `(UN)LICENSE*`, `RELEASES*`, and `CHANGELOG*` are already
    /// automatically detected and included (use [`DistMetadata::auto_includes`][] to prevent this).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<Vec<Utf8PathBuf>>,

    /// Whether to auto-include files like `README*`, `(UN)LICENSE*`, `RELEASES*`, and `CHANGELOG*`
    ///
    /// Defaults to true.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_includes: Option<bool>,

    /// The archive format to use for windows builds (defaults .zip)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_archive: Option<ZipStyle>,

    /// The archive format to use for non-windows builds (defaults .tar.xz)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unix_archive: Option<ZipStyle>,

    /// Whether to include built libraries in the release archive
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default, with = "opt_string_or_vec")]
    pub package_libraries: Option<Vec<LibraryStyle>>,

    /// Binaries for a given platform
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binaries: Option<SortedMap<String, Vec<String>>>,
}

impl ArchiveConfig {
    /// Get defaults for the given package
    pub fn defaults_for_package(_workspaces: &WorkspaceGraph, _pkg_idx: PackageIdx) -> Self {
        Self {
            include: vec![],
            auto_includes: true,
            windows_archive: ZipStyle::Zip,
            unix_archive: ZipStyle::Tar(CompressionImpl::Xzip),
            package_libraries: vec![],
            binaries: SortedMap::default(),
        }
    }
}

impl ApplyLayer for ArchiveConfig {
    type Layer = ArchiveLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            include,
            auto_includes,
            windows_archive,
            unix_archive,
            package_libraries,
            binaries,
        }: Self::Layer,
    ) {
        self.include.apply_val(include);
        self.auto_includes.apply_val(auto_includes);
        self.windows_archive.apply_val(windows_archive);
        self.unix_archive.apply_val(unix_archive);
        self.package_libraries.apply_val(package_libraries);
        self.binaries.apply_val(binaries);
    }
}
impl ApplyLayer for ArchiveLayer {
    type Layer = ArchiveLayer;
    fn apply_layer(
        &mut self,
        Self::Layer {
            include,
            auto_includes,
            windows_archive,
            unix_archive,
            package_libraries,
            binaries,
        }: Self::Layer,
    ) {
        self.include.apply_opt(include);
        self.auto_includes.apply_opt(auto_includes);
        self.windows_archive.apply_opt(windows_archive);
        self.unix_archive.apply_opt(unix_archive);
        self.package_libraries.apply_opt(package_libraries);
        self.binaries.apply_opt(binaries);
    }
}
