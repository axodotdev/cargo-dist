//! The Linkage Checker, which lets us detect what a binary dynamically links to (and why)

use std::{
    fs::{self, File},
    io::{Cursor, Read},
};

use axoasset::SourceFile;
use axoprocess::Cmd;
use camino::Utf8PathBuf;
use cargo_dist_schema::{
    AssetInfo, BuildEnvironment, DistManifest, GlibcVersion, Library, Linkage, PackageManager,
    TargetTripleRef,
};
use comfy_table::{presets::UTF8_FULL, Table};
use goblin::Object;
use mach_object::{LoadCommand, OFile};
use tracing::warn;

use crate::{config::Config, errors::*, gather_work, platforms::TARGET_HOST, Artifact, DistGraph};

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
pub fn do_linkage(cfg: &Config, args: &LinkageArgs) -> DistResult<()> {
    let manifest = if let Some(target) = args.from_json.clone() {
        let file = SourceFile::load_local(target)?;
        file.deserialize_json()?
    } else {
        let (dist, mut manifest) = gather_work(cfg)?;
        compute_linkage_assuming_local_build(&dist, &mut manifest, cfg)?;
        manifest
    };

    if args.print_output {
        eprintln!("{}", LinkageDisplay(&manifest));
    }
    if args.print_json {
        let string = serde_json::to_string_pretty(&manifest).unwrap();
        println!("{string}");
    }
    Ok(())
}

/// Assuming someone just ran `cargo dist build` on the current machine,
/// compute the linkage by checking binaries in the temp to-be-zipped dirs.
fn compute_linkage_assuming_local_build(
    dist: &DistGraph,
    manifest: &mut DistManifest,
    cfg: &Config,
) -> DistResult<()> {
    let targets = &cfg.targets;
    let artifacts = &dist.artifacts;
    let dist_dir = &dist.dist_dir;

    for target in targets {
        let artifacts: Vec<Artifact> = artifacts
            .clone()
            .into_iter()
            .filter(|r| r.target_triples.contains(target))
            .collect();

        if artifacts.is_empty() {
            eprintln!("No matching artifact for target {target}");
            continue;
        }

        for artifact in artifacts {
            let path = Utf8PathBuf::from(&dist_dir).join(format!("{}-{target}", artifact.id));

            for (bin_idx, binary_relpath) in artifact.required_binaries {
                let bin = dist.binary(bin_idx);
                let bin_path = path.join(binary_relpath);
                if !bin_path.exists() {
                    eprintln!("Binary {bin_path} missing; skipping check");
                } else {
                    let linkage = determine_linkage(&bin_path, target);
                    manifest.assets.insert(
                        bin.id.clone(),
                        AssetInfo {
                            id: bin.id.clone(),
                            name: bin.name.clone(),
                            system: dist.system_id.clone(),
                            linkage: Some(linkage),
                            target_triples: vec![target.clone()],
                        },
                    );
                }
            }
        }
    }

    Ok(())
}

/// Formatter for a DistManifest that prints the linkage human-readably
pub struct LinkageDisplay<'a>(pub &'a DistManifest);

impl std::fmt::Display for LinkageDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for asset in self.0.assets.values() {
            let Some(linkage) = &asset.linkage else {
                continue;
            };
            let name = &asset.name;
            let targets = asset.target_triples.join(", ");
            write!(f, "{name}")?;
            if !targets.is_empty() {
                write!(f, " ({targets})")?;
            }
            writeln!(f, "\n")?;
            format_linkage_table(f, linkage)?;
        }
        Ok(())
    }
}

/// Formatted human-readable output
fn format_linkage_table(f: &mut std::fmt::Formatter<'_>, linkage: &Linkage) -> std::fmt::Result {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_header(vec!["Category", "Libraries"])
        .add_row(vec![
            "System",
            linkage
                .system
                .clone()
                .into_iter()
                .map(|l| l.to_string())
                .collect::<Vec<String>>()
                .join("\n")
                .as_str(),
        ])
        .add_row(vec![
            "Homebrew",
            linkage
                .homebrew
                .clone()
                .into_iter()
                .map(|l| l.to_string())
                .collect::<Vec<String>>()
                .join("\n")
                .as_str(),
        ])
        .add_row(vec![
            "Public (unmanaged)",
            linkage
                .public_unmanaged
                .clone()
                .into_iter()
                .map(|l| l.path)
                .collect::<Vec<String>>()
                .join("\n")
                .as_str(),
        ])
        .add_row(vec![
            "Frameworks",
            linkage
                .frameworks
                .clone()
                .into_iter()
                .map(|l| l.path)
                .collect::<Vec<String>>()
                .join("\n")
                .as_str(),
        ])
        .add_row(vec![
            "Other",
            linkage
                .other
                .clone()
                .into_iter()
                .map(|l| l.to_string())
                .collect::<Vec<String>>()
                .join("\n")
                .as_str(),
        ]);
    write!(f, "{table}")
}

/// Create a homebrew library for the given path
pub fn library_from_homebrew(library: String) -> Library {
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
        let mut package = stripped.split('/').next().unwrap().to_owned();

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

        Library {
            path: library,
            source: Some(package.to_owned()),
            package_manager: Some(PackageManager::Homebrew),
        }
    } else {
        Library {
            path: library,
            source: None,
            package_manager: None,
        }
    }
}

/// Create an apt library for the given path
pub fn library_from_apt(library: String) -> DistResult<Library> {
    // We can't get this information on other OSs
    if std::env::consts::OS != "linux" {
        return Ok(Library {
            path: library,
            source: None,
            package_manager: None,
        });
    }

    let process = Cmd::new("dpkg", "get linkage info from dpkg")
        .arg("--search")
        .arg(&library)
        .output();
    match process {
        Ok(output) => {
            let output = String::from_utf8(output.stdout)?;

            let package = output.split(':').next().unwrap();
            let source = if package.is_empty() {
                None
            } else {
                Some(package.to_owned())
            };
            let package_manager = if source.is_some() {
                Some(PackageManager::Apt)
            } else {
                None
            };

            Ok(Library {
                path: library,
                source,
                package_manager,
            })
        }
        // Couldn't find a package for this file
        Err(_) => Ok(Library {
            path: library,
            source: None,
            package_manager: None,
        }),
    }
}

fn do_otool(path: &Utf8PathBuf) -> DistResult<Vec<String>> {
    let mut libraries = vec![];

    let mut f = File::open(path)?;
    let mut buf = vec![];
    let size = f.read_to_end(&mut buf).unwrap();
    let mut cur = Cursor::new(&buf[..size]);
    if let Ok(OFile::MachFile {
        header: _,
        commands,
    }) = OFile::parse(&mut cur)
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

    // We ignore the status here because for whatever reason arm64 glibc ldd can decide
    // to return non-zero status on binaries with no dynamic linkage (e.g. musl-static).
    // This was observed both in arm64 ubuntu and asahi (both glibc ldd).
    // x64 glibc ldd is perfectly fine with this and returns 0, so... *shrug* compilers!
    let output = Cmd::new("ldd", "get linkage info from ldd")
        .arg(path)
        .check(false)
        .output()?;

    let result = String::from_utf8_lossy(&output.stdout).to_string();
    let lines = result.trim_end().split('\n');

    for line in lines {
        let line = line.trim();

        // There's no dynamic linkage at all; we can safely break,
        // there will be nothing useful to us here.
        if line.starts_with("not a dynamic executable") || line.starts_with("statically linked") {
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
        // Static libraries link against nothing
        Object::Archive(_) => Ok(vec![]),
        _ => Err(DistError::LinkageCheckUnsupportedBinary),
    }
}

/// Get the linkage for a single binary
///
/// If linkage fails for any reason we warn and return the default empty linkage
pub fn determine_linkage(path: &Utf8PathBuf, target: &TargetTripleRef) -> Linkage {
    match try_determine_linkage(path, target) {
        Ok(linkage) => linkage,
        Err(e) => {
            warn!("Skipping linkage for {path}:\n{:?}", miette::Report::new(e));
            Linkage::default()
        }
    }
}

/// Get the linkage for a single binary
fn try_determine_linkage(path: &Utf8PathBuf, target: &TargetTripleRef) -> DistResult<Linkage> {
    let libraries = if target.is_darwin() {
        do_otool(path)?
    } else if target.is_linux() {
        // Currently can only be run on Linux
        if !TARGET_HOST.is_linux() {
            return Err(DistError::LinkageCheckInvalidOS {
                host: TARGET_HOST.to_owned(),
                target: target.to_owned(),
            });
        }
        do_ldd(path)?
    } else if target.is_windows() {
        do_pe(path)?
    } else {
        return Err(DistError::LinkageCheckUnsupportedBinary);
    };

    let mut linkage = Linkage {
        system: Default::default(),
        homebrew: Default::default(),
        public_unmanaged: Default::default(),
        frameworks: Default::default(),
        other: Default::default(),
    };
    for library in libraries {
        if library.starts_with("/opt/homebrew") {
            linkage
                .homebrew
                .insert(library_from_homebrew(library.clone()));
        } else if library.starts_with("/usr/lib") || library.starts_with("/lib") {
            linkage.system.insert(library_from_apt(library.clone())?);
        } else if library.starts_with("/System/Library/Frameworks")
            || library.starts_with("/Library/Frameworks")
        {
            linkage.frameworks.insert(Library::new(library.clone()));
        } else if library.starts_with("/usr/local") {
            if std::fs::canonicalize(&library)?.starts_with("/usr/local/Cellar") {
                linkage
                    .homebrew
                    .insert(library_from_homebrew(library.clone()));
            } else {
                linkage
                    .public_unmanaged
                    .insert(Library::new(library.clone()));
            }
        } else {
            linkage.other.insert(library_from_apt(library.clone())?);
        }
    }

    Ok(linkage)
}

/// Determine the build environment on the current host
/// This should be done local to the builder!
pub fn determine_build_environment(target: &TargetTripleRef) -> BuildEnvironment {
    if target.is_darwin() {
        determine_macos_build_environment().unwrap_or(BuildEnvironment::Indeterminate)
    } else if target.is_linux() {
        determine_linux_build_environment().unwrap_or(BuildEnvironment::Indeterminate)
    } else if target.is_windows() {
        BuildEnvironment::Windows
    } else {
        BuildEnvironment::Indeterminate
    }
}

fn determine_linux_build_environment() -> DistResult<BuildEnvironment> {
    // If we're running this cross-host somehow, we should return an
    // indeterminate result here
    if std::env::consts::OS != "linux" {
        return Ok(BuildEnvironment::Indeterminate);
    }

    let mut cmd = Cmd::new("ldd", "determine glibc version");
    cmd.arg("--version");
    let output = cmd.output()?;
    let output_str = String::from_utf8(output.stdout)?;
    let first_line = output_str.lines().next().unwrap_or(&output_str).trim_end();
    // Running on a system without glibc at all
    let glibc_version = if !first_line.contains("GNU libc") && !first_line.contains("GLIBC") {
        None
    } else {
        // Formats observed in the wild:
        // ldd (Ubuntu GLIBC 2.35-0ubuntu3.8) 2.35 (Ubuntu 22.04)
        // ldd (Debian GLIBC 2.36-9+deb12u7) 2.36 (Debian)
        // ldd (GNU libc) 2.39 (Fedora)
        first_line
            .split(' ')
            .last()
            .and_then(|s| s.split_once('.').map(glibc_from_tuple))
            .transpose()?
    };

    Ok(BuildEnvironment::Linux { glibc_version })
}

fn glibc_from_tuple(versions: (&str, &str)) -> Result<GlibcVersion, DistError> {
    let major = versions.0.parse::<u64>()?;
    let series = versions.1.parse::<u64>()?;

    Ok(GlibcVersion { major, series })
}

fn determine_macos_build_environment() -> DistResult<BuildEnvironment> {
    // If we're running this cross-host somehow, we should return an
    // indeterminate result here
    if std::env::consts::OS != "macos" {
        return Ok(BuildEnvironment::Indeterminate);
    }

    let mut cmd = Cmd::new("sw_vers", "determine OS version");
    cmd.arg("-productVersion");
    let output = cmd.output()?;
    let os_version = String::from_utf8(output.stdout)?.trim_end().to_owned();

    Ok(BuildEnvironment::MacOS { os_version })
}
