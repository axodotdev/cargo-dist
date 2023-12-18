//! The Linkage Checker, which lets us detect what a binary dynamically links to (and why)

use std::{
    fs::{self, File},
    io::{Cursor, Read},
};

use axoasset::SourceFile;
use axoprocess::Cmd;
use camino::Utf8PathBuf;
use cargo_dist_schema::DistManifest;
use comfy_table::{presets::UTF8_FULL, Table};
use goblin::Object;
use mach_object::{LoadCommand, OFile};
use serde::{Deserialize, Serialize};

use crate::{config::Config, errors::*, gather_work, Artifact, DistGraph};

/// Arguments for `cargo dist linkage` ([`do_linkage][])
#[derive(Debug)]
pub struct LinkageArgs {
    /// Print human-readable output
    pub print_output: bool,
    /// Print output as JSON
    pub print_json: bool,
    /// Read linkage data from JSON rather than performing a live check
    pub from_json: Option<String>,
}

/// Determinage dynamic linkage of built artifacts (impl of `cargo dist linkage`)
pub fn do_linkage(cfg: &Config, args: &LinkageArgs) -> Result<()> {
    let (dist, _manifest) = gather_work(cfg)?;

    let reports: Vec<Linkage> = if let Some(target) = args.from_json.clone() {
        let file = SourceFile::load_local(target)?;
        file.deserialize_json()?
    } else {
        fetch_linkage(cfg.targets.clone(), dist.artifacts, dist.dist_dir)?
    };

    if args.print_output {
        for report in &reports {
            eprintln!("{}", report.report());
        }
    }
    if args.print_json {
        let j = serde_json::to_string(&reports).unwrap();
        println!("{}", j);
    }

    Ok(())
}

/// Compute the linkage of local builds and add them to the DistManifest
pub fn add_linkage_to_manifest(
    cfg: &Config,
    dist: &DistGraph,
    manifest: &mut DistManifest,
) -> Result<()> {
    let linkage = fetch_linkage(
        cfg.targets.clone(),
        dist.artifacts.clone(),
        dist.dist_dir.clone(),
    )?;

    manifest
        .linkage
        .extend(linkage.iter().map(|l| l.to_schema()));
    Ok(())
}

fn fetch_linkage(
    targets: Vec<String>,
    artifacts: Vec<Artifact>,
    dist_dir: Utf8PathBuf,
) -> DistResult<Vec<Linkage>> {
    let mut reports = vec![];

    for target in targets {
        let artifacts: Vec<Artifact> = artifacts
            .clone()
            .into_iter()
            .filter(|r| r.target_triples.contains(&target))
            .collect();

        if artifacts.is_empty() {
            eprintln!("No matching artifact for target {target}");
            continue;
        }

        for artifact in artifacts {
            let path = Utf8PathBuf::from(&dist_dir).join(format!("{}-{target}", artifact.id));

            for (_, binary) in artifact.required_binaries {
                let bin_path = path.join(binary);
                if !bin_path.exists() {
                    eprintln!("Binary {bin_path} missing; skipping check");
                } else {
                    reports.push(determine_linkage(&bin_path, &target)?);
                }
            }
        }
    }

    Ok(reports)
}

/// Information about dynamic libraries used by a binary
#[derive(Debug, Deserialize, Serialize)]
pub struct Linkage {
    /// The filename of the binary
    pub binary: String,
    /// The target triple for which the binary was built
    pub target: String,
    /// Libraries included with the operating system
    pub system: Vec<Library>,
    /// Libraries provided by the Homebrew package manager
    pub homebrew: Vec<Library>,
    /// Public libraries not provided by the system and not managed by any package manager
    pub public_unmanaged: Vec<Library>,
    /// Libraries which don't fall into any other categories
    pub other: Vec<Library>,
    /// Frameworks, only used on macOS
    pub frameworks: Vec<Library>,
}

impl Linkage {
    /// Formatted human-readable output
    pub fn report(&self) -> String {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .set_header(vec!["Category", "Libraries"])
            .add_row(vec![
                "System",
                self.system
                    .clone()
                    .into_iter()
                    .map(|l| l.to_string_pretty())
                    .collect::<Vec<String>>()
                    .join("\n")
                    .as_str(),
            ])
            .add_row(vec![
                "Homebrew",
                self.homebrew
                    .clone()
                    .into_iter()
                    .map(|l| l.to_string_pretty())
                    .collect::<Vec<String>>()
                    .join("\n")
                    .as_str(),
            ])
            .add_row(vec![
                "Public (unmanaged)",
                self.public_unmanaged
                    .clone()
                    .into_iter()
                    .map(|l| l.path)
                    .collect::<Vec<String>>()
                    .join("\n")
                    .as_str(),
            ])
            .add_row(vec![
                "Frameworks",
                self.frameworks
                    .clone()
                    .into_iter()
                    .map(|l| l.path)
                    .collect::<Vec<String>>()
                    .join("\n")
                    .as_str(),
            ])
            .add_row(vec![
                "Other",
                self.other
                    .clone()
                    .into_iter()
                    .map(|l| l.to_string_pretty())
                    .collect::<Vec<String>>()
                    .join("\n")
                    .as_str(),
            ]);

        let s = format!(
            r#"{} ({}):

{table}"#,
            self.binary, self.target,
        );

        s.to_owned()
    }

    fn to_schema(&self) -> cargo_dist_schema::Linkage {
        cargo_dist_schema::Linkage {
            binary: self.binary.clone(),
            target: self.target.clone(),
            system: self.system.iter().map(|s| s.to_schema()).collect(),
            homebrew: self.homebrew.iter().map(|s| s.to_schema()).collect(),
            public_unmanaged: self
                .public_unmanaged
                .iter()
                .map(|s| s.to_schema())
                .collect(),
            other: self.other.iter().map(|s| s.to_schema()).collect(),
            frameworks: self.frameworks.iter().map(|s| s.to_schema()).collect(),
        }
    }

    /// Constructs a Linkage from a cargo_dist_schema::Linkage
    pub fn from_schema(other: &cargo_dist_schema::Linkage) -> Self {
        Self {
            binary: other.binary.clone(),
            target: other.target.clone(),
            system: other.system.iter().map(Library::from_schema).collect(),
            homebrew: other.homebrew.iter().map(Library::from_schema).collect(),
            public_unmanaged: other
                .public_unmanaged
                .iter()
                .map(Library::from_schema)
                .collect(),
            other: other.other.iter().map(Library::from_schema).collect(),
            frameworks: other.frameworks.iter().map(Library::from_schema).collect(),
        }
    }
}

/// Represents a dynamic library located somewhere on the system
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Library {
    /// The path to the library; on platforms without that information, it will be a basename instead
    pub path: String,
    /// The package from which a library comes, if relevant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl Library {
    fn new(library: String) -> Self {
        Self {
            path: library,
            source: None,
        }
    }

    fn to_schema(&self) -> cargo_dist_schema::Library {
        cargo_dist_schema::Library {
            path: self.path.clone(),
            source: self.source.clone(),
        }
    }

    fn from_schema(other: &cargo_dist_schema::Library) -> Self {
        Self {
            path: other.path.clone(),
            source: other.source.clone(),
        }
    }

    fn from_homebrew(library: String) -> Self {
        // Doesn't currently support Homebrew installations in
        // non-default locations
        let brew_prefix = if library.starts_with("/opt/homebrew/opt/") {
            Some("/opt/homebrew/opt/")
        } else if library.starts_with("/usr/local/opt/") {
            Some("/usr/local/opt/")
        } else {
            None
        };

        if let Some(prefix) = brew_prefix {
            let cloned = library.clone();
            let stripped = cloned.strip_prefix(prefix).unwrap();
            let mut package = stripped.split('/').nth(0).unwrap().to_owned();

            // The path alone isn't enough to determine the tap the formula
            // came from. If the install receipt exists, we can use it to
            // get the name of the source tap.
            let receipt = Utf8PathBuf::from(&prefix)
                .join(&package)
                .join("INSTALL_RECEIPT.json");

            // If the receipt doesn't exist or can't be loaded, that's not an
            // error; we can fall back to the package basename we parsed out
            // of the path.
            if receipt.exists() {
                let _ = SourceFile::load_local(&receipt)
                    .and_then(|file| file.deserialize_json())
                    .map(|parsed: serde_json::Value| {
                        if let Some(tap) = parsed["source"]["tap"].as_str() {
                            if tap != "homebrew/core" {
                                package = format!("{tap}/{package}");
                            }
                        }
                    });
            }

            Self {
                path: library,
                source: Some(package.to_owned()),
            }
        } else {
            Self {
                path: library,
                source: None,
            }
        }
    }

    fn maybe_apt(library: String) -> DistResult<Self> {
        // We can't get this information on other OSs
        if std::env::consts::OS != "linux" {
            return Ok(Self {
                path: library,
                source: None,
            });
        }

        let process = Cmd::new("dpkg", "get linkage info from dpkg")
            .arg("--search")
            .arg(&library)
            .output();
        match process {
            Ok(output) => {
                let output = String::from_utf8(output.stdout)?;

                let package = output.split(':').nth(0).unwrap();
                let source = if package.is_empty() {
                    None
                } else {
                    Some(package.to_owned())
                };

                Ok(Self {
                    path: library,
                    source,
                })
            }
            // Couldn't find a package for this file
            Err(_) => Ok(Self {
                path: library,
                source: None,
            }),
        }
    }

    fn to_string_pretty(&self) -> String {
        if let Some(package) = &self.source {
            format!("{} ({package})", self.path).to_owned()
        } else {
            self.path.clone()
        }
    }
}

fn do_otool(path: &Utf8PathBuf) -> DistResult<Vec<String>> {
    let mut libraries = vec![];

    let mut f = File::open(path)?;
    let mut buf = vec![];
    let size = f.read_to_end(&mut buf).unwrap();
    let mut cur = Cursor::new(&buf[..size]);
    if let OFile::MachFile {
        header: _,
        commands,
    } = OFile::parse(&mut cur).unwrap()
    {
        let commands = commands
            .iter()
            .map(|load| load.command())
            .cloned()
            .collect::<Vec<LoadCommand>>();

        for command in commands {
            match command {
                LoadCommand::IdDyLib(ref dylib)
                | LoadCommand::LoadDyLib(ref dylib)
                | LoadCommand::LoadWeakDyLib(ref dylib)
                | LoadCommand::ReexportDyLib(ref dylib)
                | LoadCommand::LoadUpwardDylib(ref dylib)
                | LoadCommand::LazyLoadDylib(ref dylib) => {
                    libraries.push(dylib.name.to_string());
                }
                _ => {}
            }
        }
    }

    Ok(libraries)
}

fn do_ldd(path: &Utf8PathBuf) -> DistResult<Vec<String>> {
    let mut libraries = vec![];

    let output = Cmd::new("ldd", "get linkage info from ldd")
        .arg(path)
        .output()?;

    let result = String::from_utf8_lossy(&output.stdout).to_string();
    let lines = result.trim_end().split('\n');

    for line in lines {
        let line = line.trim();

        // There's no dynamic linkage at all; we can safely break,
        // there will be nothing useful to us here.
        if line.starts_with("not a dynamic executable") {
            break;
        }

        // Not a library that actually concerns us
        if line.starts_with("linux-vdso") {
            continue;
        }

        // Format: libname.so.1 => /path/to/libname.so.1 (address)
        if let Some(path) = line.split(" => ").nth(1) {
            // This may be a symlink rather than the actual underlying library;
            // we resolve the symlink here so that we return the real paths,
            // making it easier to map them to their packages later.
            let lib = (path.split(' ').next().unwrap()).to_owned();
            let realpath = fs::canonicalize(&lib)?;
            libraries.push(realpath.to_string_lossy().to_string());
        } else {
            continue;
        }
    }

    Ok(libraries)
}

fn do_pe(path: &Utf8PathBuf) -> DistResult<Vec<String>> {
    let buf = std::fs::read(path)?;
    match Object::parse(&buf)? {
        Object::PE(pe) => Ok(pe.libraries.into_iter().map(|s| s.to_owned()).collect()),
        _ => Err(DistError::LinkageCheckUnsupportedBinary {}),
    }
}

fn determine_linkage(path: &Utf8PathBuf, target: &str) -> DistResult<Linkage> {
    let libraries = match target {
        // Can be run on any OS
        "i686-apple-darwin" | "x86_64-apple-darwin" | "aarch64-apple-darwin" => do_otool(path)?,
        "i686-unknown-linux-gnu"
        | "x86_64-unknown-linux-gnu"
        | "aarch64-unknown-linux-gnu"
        | "i686-unknown-linux-musl"
        | "x86_64-unknown-linux-musl"
        | "aarch64-unknown-linux-musl" => {
            // Currently can only be run on Linux
            if std::env::consts::OS != "linux" {
                return Err(DistError::LinkageCheckInvalidOS {
                    host: std::env::consts::OS.to_owned(),
                    target: target.to_owned(),
                });
            }
            do_ldd(path)?
        }
        // Can be run on any OS
        "i686-pc-windows-msvc" | "x86_64-pc-windows-msvc" | "aarch64-pc-windows-msvc" => {
            do_pe(path)?
        }
        _ => return Err(DistError::LinkageCheckUnsupportedBinary {}),
    };

    let mut linkage = Linkage {
        binary: path.file_name().unwrap().to_owned(),
        target: target.to_owned(),
        system: vec![],
        homebrew: vec![],
        public_unmanaged: vec![],
        frameworks: vec![],
        other: vec![],
    };
    for library in libraries {
        if library.starts_with("/opt/homebrew") {
            linkage
                .homebrew
                .push(Library::from_homebrew(library.clone()));
        } else if library.starts_with("/usr/lib") || library.starts_with("/lib") {
            linkage.system.push(Library::maybe_apt(library.clone())?);
        } else if library.starts_with("/System/Library/Frameworks")
            || library.starts_with("/Library/Frameworks")
        {
            linkage.frameworks.push(Library::new(library.clone()));
        } else if library.starts_with("/usr/local") {
            if std::fs::canonicalize(&library)?.starts_with("/usr/local/Cellar") {
                linkage
                    .homebrew
                    .push(Library::from_homebrew(library.clone()));
            } else {
                linkage.public_unmanaged.push(Library::new(library.clone()));
            }
        } else {
            linkage.other.push(Library::maybe_apt(library.clone())?);
        }
    }

    Ok(linkage)
}
