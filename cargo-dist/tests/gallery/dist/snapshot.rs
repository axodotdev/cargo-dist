use super::*;

pub struct Snapshots {
    settings: insta::Settings,
    name: String,
    payload: String,
}

impl DistResult {
    // Run cargo-insta on everything we care to snapshot
    pub fn snapshot(&self) -> Result<Snapshots> {
        // We make a single uber-snapshot for both scripts to avoid the annoyances of having multiple snapshots
        // in one test (necessitating rerunning it multiple times or passing special flags to get all the changes)
        let mut snapshots = String::new();

        for app in &self.apps {
            append_snapshot_file(
                &mut snapshots,
                app.shell_installer_path
                    .as_deref()
                    .and_then(|p| p.file_name())
                    .unwrap_or_default(),
                app.shell_installer_path.as_deref(),
            )?;
            append_snapshot_file(
                &mut snapshots,
                app.homebrew_installer_path
                    .as_deref()
                    .and_then(|p| p.file_name())
                    .unwrap_or_default(),
                app.homebrew_installer_path.as_deref(),
            )?;
            append_snapshot_file(
                &mut snapshots,
                app.powershell_installer_path
                    .as_deref()
                    .and_then(|p| p.file_name())
                    .unwrap_or_default(),
                app.powershell_installer_path.as_deref(),
            )?;
            append_snapshot_tarball(
                &mut snapshots,
                app.npm_installer_package_path
                    .as_deref()
                    .and_then(|p| p.file_name())
                    .unwrap_or_default(),
                app.npm_installer_package_path.as_deref(),
            )?;
        }

        Ok(Snapshots {
            settings: snapshot_settings_with_gallery_filter(),
            name: self.test_name.to_owned(),
            payload: snapshots,
        })
    }
}

impl PlanResult {
    // Run cargo-insta on everything we care to snapshot
    pub fn snapshot(&self) -> Result<Snapshots> {
        // We make a single uber-snapshot for both scripts to avoid the annoyances of having multiple snapshots
        // in one test (necessitating rerunning it multiple times or passing special flags to get all the changes)
        let mut snapshots = String::new();

        append_snapshot_string(&mut snapshots, "dist-manifest.json", &self.raw_json)?;

        Ok(Snapshots {
            settings: snapshot_settings_with_gallery_filter(),
            name: self.test_name.to_owned(),
            payload: snapshots,
        })
    }
}

impl GenerateResult {
    // Run cargo-insta on everything we care to snapshot
    pub fn snapshot(&self) -> Result<Snapshots> {
        // We make a single uber-snapshot for both scripts to avoid the annoyances of having multiple snapshots
        // in one test (necessitating rerunning it multiple times or passing special flags to get all the changes)
        let mut snapshots = String::new();

        eprintln!("{:?}", self.github_ci_path);
        append_snapshot_file(
            &mut snapshots,
            self.github_ci_path
                .as_deref()
                .and_then(|p| p.file_name())
                .unwrap_or_default(),
            self.github_ci_path.as_deref(),
        )?;

        append_snapshot_file(&mut snapshots, "main.wxs", self.wxs_path.as_deref())?;

        Ok(Snapshots {
            settings: snapshot_settings_with_gallery_filter(),
            name: self.test_name.to_owned(),
            payload: snapshots,
        })
    }
}

impl Snapshots {
    pub fn snap(self) {
        self.settings.bind(|| {
            insta::assert_snapshot!(self.name, self.payload);
        })
    }

    pub fn join(mut self, other: Self) -> Self {
        self.payload.push_str(&other.payload);
        self
    }
}

pub fn snapshot_settings() -> insta::Settings {
    let mut settings = insta::Settings::clone_current();
    let snapshot_dir = Utf8Path::new(ROOT_DIR).join("tests").join("snapshots");
    settings.set_snapshot_path(snapshot_dir);
    settings.set_prepend_module_to_snapshot(false);
    settings
}

pub fn snapshot_settings_with_version_filter() -> insta::Settings {
    let mut settings = snapshot_settings();
    settings.add_filter(
        r"\d+\.\d+\.\d+(\-prerelease\d*)?(\.\d+)?",
        "1.0.0-FAKEVERSION",
    );
    settings
}

/// Only filter parts that are specific to the toolchains being used to build the result
///
/// This is used for checking gallery entries
pub fn snapshot_settings_with_gallery_filter() -> insta::Settings {
    let mut settings = snapshot_settings();
    settings.add_filter(r#""dist_version": .*"#, r#""dist_version": "CENSORED","#);
    settings.add_filter(
        r#""cargo_version_line": .*"#,
        r#""cargo_version_line": "CENSORED""#,
    );
    settings.add_filter(
        r"cargo-dist/releases/download/v\d+\.\d+\.\d+(\-prerelease\d*)?(\.\d+)?/",
        "cargo-dist/releases/download/vSOME_VERSION/",
    );
    settings.add_filter(r#"sha256 ".*""#, r#"sha256 "CENSORED""#);
    settings.add_filter(r#""sha256": .*"#, r#""sha256": "CENSORED""#);
    settings.add_filter(r#""sha512": .*"#, r#""sha512": "CENSORED""#);
    settings.add_filter(r#""version":"[a-zA-Z\.0-9\-]*""#, r#""version":"CENSORED""#);
    settings
}

/// Filter anything that will regularly change in the process of a release
///
/// This is used for checking `main` against itself.
#[allow(dead_code)]
pub fn snapshot_settings_with_dist_manifest_filter() -> insta::Settings {
    let mut settings = snapshot_settings_with_version_filter();
    settings.add_filter(
        r#""announcement_tag": .*"#,
        r#""announcement_tag": "CENSORED","#,
    );
    settings.add_filter(
        r#""announcement_title": .*"#,
        r#""announcement_title": "CENSORED""#,
    );
    settings.add_filter(
        r#""announcement_changelog": .*"#,
        r#""announcement_changelog": "CENSORED""#,
    );
    settings.add_filter(
        r#""announcement_github_body": .*"#,
        r#""announcement_github_body": "CENSORED""#,
    );
    settings.add_filter(
        r#""announcement_is_prerelease": .*"#,
        r#""announcement_is_prerelease": "CENSORED""#,
    );
    settings.add_filter(
        r#""cargo_version_line": .*"#,
        r#""cargo_version_line": "CENSORED""#,
    );
    settings.add_filter(r#""sha256": .*"#, r#""sha256": "CENSORED""#);
    settings.add_filter(r#""sha512": .*"#, r#""sha512": "CENSORED""#);

    settings
}

fn append_snapshot_tarball(
    out: &mut String,
    name: &str,
    src_path: Option<&Utf8Path>,
) -> Result<()> {
    use std::io::Read;

    // Skip snapshotting this file if absent
    let Some(src_path) = src_path else {
        return Ok(());
    };

    // We shove everything in a BTreeMap to keep ordering stable
    let mut results = BTreeMap::new();

    let file = LocalAsset::load_bytes(src_path)?;
    let gz_decoder = flate2::read::GzDecoder::new(&file[..]);
    let mut tar_decoder = tar::Archive::new(gz_decoder);
    let entries = tar_decoder.entries().expect("couldn't read tar");
    for entry in entries {
        let mut entry = entry.expect("couldn't read tar entry");
        if entry.header().entry_type() == tar::EntryType::Regular {
            let path = entry
                .path()
                .expect("couldn't get tarred file's path")
                .to_string_lossy()
                .into_owned();
            let mut val = String::new();
            entry
                .read_to_string(&mut val)
                .expect("couldn't read tarred file to string");
            results.insert(path, val);
        }
    }

    for (path, val) in &results {
        append_snapshot_string(out, &format!("{name}/{path}"), val)?;
    }
    Ok(())
}

fn append_snapshot_file(out: &mut String, name: &str, src_path: Option<&Utf8Path>) -> Result<()> {
    // Skip snapshotting this file if absent
    let Some(src_path) = src_path else {
        return Ok(());
    };

    let src = axoasset::LocalAsset::load_string(src_path)?;
    append_snapshot_string(out, name, &src)
}

fn append_snapshot_string(out: &mut String, name: &str, val: &str) -> Result<()> {
    use std::fmt::Write;

    writeln!(out, "================ {name} ================").unwrap();
    writeln!(out, "{val}").unwrap();
    Ok(())
}
