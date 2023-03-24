use std::path::PathBuf;

use axo_project::WorkspaceInfo;
use camino::Utf8PathBuf;

fn main() -> Result<(), miette::Report> {
    // Take an optional "path to search from" as the first argument
    // Otherwise use current working directory
    let mut args = std::env::args_os();
    let _bin_name = args.next();
    let start_path = args.next().map(PathBuf::from);

    let start_dir = start_path
        .unwrap_or_else(|| std::env::current_dir().expect("couldn't get current working dir!?"));
    let start_dir = Utf8PathBuf::from_path_buf(start_dir).expect("project path isn't utf8!?");

    if let Some(project) = axo_project::get_project(&start_dir) {
        print_project(&project);
    } else {
        eprintln!("Couldn't find a project in {}", start_dir);
    }
    Ok(())
}

fn print_project(project: &WorkspaceInfo) {
    let disabled_sty = console::Style::new().dim();
    let enabled_sty = console::Style::new();

    for (_, pkg) in project.packages() {
        let pkg_name = &pkg.name;

        // Determine if this package's binaries should be Released
        let mut disabled_reason = None;
        if pkg.binaries.is_empty() {
            // Nothing to publish if there's no binaries!
            disabled_reason = Some("no binaries".to_owned());
        /*
        } else if let Some(do_dist) = pkg.config.dist {
            // If [metadata.dist].dist is explicitly set, respect it!
            if !do_dist {
                disabled_reason = Some("dist = false".to_owned());
            }
         */
        } else if !pkg.publish {
            // Otherwise defer to Cargo's `publish = false`
            disabled_reason = Some("publish = false".to_owned());
            /*
            } else if let Some(id) = &announcing_package {
                // If we're announcing a package, reject every other package
                if pkg_id != id {
                    disabled_reason = Some(format!(
                        "didn't match tag {}",
                        announcement_tag.as_ref().unwrap()
                    ));
                }
            } else if let Some(ver) = &announcing_version {
                if &pkg.version != ver {
                    disabled_reason = Some(format!(
                        "didn't match tag {}",
                        announcement_tag.as_ref().unwrap()
                    ));
                }
             */
        }

        // Report our conclusion/discoveries
        let sty;
        if let Some(reason) = &disabled_reason {
            sty = &disabled_sty;
            eprintln!("  {}", sty.apply_to(format!("{pkg_name} ({reason})")));
        } else {
            sty = &enabled_sty;
            eprintln!("  {}", sty.apply_to(pkg_name));
        }

        // Report each binary and potentially add it to the Release for this package
        let mut rust_binaries = vec![];
        for binary in &pkg.binaries {
            eprintln!("    {}", sty.apply_to(format!("[bin] {}", binary)));
            // In the future might want to allow this to be granular for each binary
            if disabled_reason.is_none() {
                rust_binaries.push(binary);
            }
        }

        // If any binaries were accepted for this package, it's a Release!
        if !rust_binaries.is_empty() {
            // rust_releases.push((*pkg_id, rust_binaries));
        }
    }
    eprintln!();
}
