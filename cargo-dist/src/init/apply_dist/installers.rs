use super::helpers::*;
use crate::config::v1::layer::BoolOr;
use axoasset::toml_edit::{self, Item, Table};

use crate::config::v1::installers::{
    homebrew::HomebrewInstallerLayer, msi::MsiInstallerLayer, npm::NpmInstallerLayer,
    pkg::PkgInstallerLayer, powershell::PowershellInstallerLayer, shell::ShellInstallerLayer,
    CommonInstallerLayer, InstallerLayer,
};

pub fn apply(table: &mut toml_edit::Table, installers: &Option<InstallerLayer>) {
    let Some(installers) = installers else {
        return;
    };
    let installers_table = table
        .entry("installers")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.installers] should be a table");

    apply_installers_common(installers_table, &installers.common);

    if let Some(homebrew) = &installers.homebrew {
        match homebrew {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "homebrew",
                    "# Whether to build a Homebrew installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_homebrew(installers_table, v);
            }
        }
    }

    if let Some(msi) = &installers.msi {
        match msi {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "msi",
                    "# Whether to build an MSI installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_msi(installers_table, v);
            }
        }
    }

    if let Some(npm) = &installers.npm {
        match npm {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "npm",
                    "# Whether to build an NPM installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_npm(installers_table, v);
            }
        }
    }

    if let Some(powershell) = &installers.powershell {
        match powershell {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "powershell",
                    "# Whether to build a PowerShell installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_powershell(installers_table, v);
            }
        }
    }

    if let Some(shell) = &installers.shell {
        match shell {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "shell",
                    "# Whether to build a Shell installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_shell(installers_table, v);
            }
        }
    }

    if let Some(pkg) = &installers.pkg {
        match pkg {
            BoolOr::Bool(b) => {
                apply_optional_value(
                    installers_table,
                    "pkg",
                    "# Whether to build a Mac .pkg installer\n",
                    Some(*b),
                );
            }
            BoolOr::Val(v) => {
                apply_installers_pkg(installers_table, v);
            }
        }
    }

    // installer.updater: Option<Bool>
    // installer.always_use_latest_updater: Option<bool>
    apply_optional_value(
        installers_table,
        "updater",
        "# Whether to install an updater program alongside the software\n",
        installers.updater,
    );

    apply_optional_value(
        installers_table,
        "always-use-latest-updater",
        "# Whether to always use the latest updater version instead of a fixed version\n",
        installers.always_use_latest_updater,
    );

    // Finalize the table
    installers_table
        .decor_mut()
        .set_prefix("\n# Installer configuration for dist\n");
}

fn apply_installers_common(table: &mut toml_edit::Table, common: &CommonInstallerLayer) {
    apply_string_or_list(
        table,
        "install-path",
        "# Path that installers should place binaries in\n",
        common.install_path.as_ref(),
    );

    apply_optional_value(
        table,
        "install-success-msg",
        "# Custom message to display on successful install\n",
        common.install_success_msg.as_deref(),
    );

    apply_string_or_list(
        table,
        "install-libraries",
        "# Which kinds of packaged libraries to install\n",
        common.install_libraries.as_ref(),
    );

    // [dist.ci.installers.bin-aliases] is not reformatted due to complexity.
    skip_string_list(
        table,
        "bin-aliases",
        "# Aliases to install for generated binaries\n",
        common.bin_aliases.as_ref(),
    );
}

fn apply_installers_homebrew(
    installers_table: &mut toml_edit::Table,
    homebrew: &HomebrewInstallerLayer,
) {
    let homebrew_table = installers_table
        .entry("homebrew")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.installers.homebrew] should be a table");

    apply_installers_common(homebrew_table, &homebrew.common);

    apply_optional_value(
        homebrew_table,
        "tap",
        "# A GitHub repo to push Homebrew formulas to\n",
        homebrew.tap.clone(),
    );

    apply_optional_value(
        homebrew_table,
        "formula",
        "# Customize the Homebrew formula name\n",
        homebrew.formula.clone(),
    );

    // Finalize the table
    homebrew_table
        .decor_mut()
        .set_prefix("\n# Configuration for the Homebrew installer\n");
}

fn apply_installers_msi(installers_table: &mut toml_edit::Table, msi: &MsiInstallerLayer) {
    let msi_table = installers_table
        .entry("msi")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.installers.msi] should be a table");

    apply_installers_common(msi_table, &msi.common);

    // There are no items under MsiInstallerConfig aside from `msi.common`.

    msi_table
        .decor_mut()
        .set_prefix("\n# Configuration for the MSI installer\n");
}

fn apply_installers_npm(installers_table: &mut toml_edit::Table, npm: &NpmInstallerLayer) {
    let npm_table = installers_table
        .entry("npm")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.installers.npm] should be a table");

    apply_installers_common(npm_table, &npm.common);

    apply_optional_value(
        npm_table,
        "package",
        "# The name of the npm package\n",
        npm.package.as_deref(),
    );

    apply_optional_value(
        npm_table,
        "scope",
        "# The namespace to use when publishing this package to the npm registry\n",
        npm.scope.as_deref(),
    );

    npm_table
        .decor_mut()
        .set_prefix("\n# Configuration for the NPM installer\n");
}

fn apply_installers_powershell(
    installers_table: &mut toml_edit::Table,
    powershell: &PowershellInstallerLayer,
) {
    let powershell_table = installers_table
        .entry("powershell")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.installers.powershell] should be a table");

    apply_installers_common(powershell_table, &powershell.common);

    // Finalize the table
    installers_table
        .decor_mut()
        .set_prefix("\n# Configuration for the Windows PowerShell installer\n");
}

fn apply_installers_shell(installers_table: &mut toml_edit::Table, shell: &ShellInstallerLayer) {
    let shell_table = installers_table
        .entry("shell")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.installers.shell] should be a table");

    apply_installers_common(shell_table, &shell.common);

    // Finalize the table
    installers_table
        .decor_mut()
        .set_prefix("\n# Configuration for the *nix shell installer\n");
}

fn apply_installers_pkg(installers_table: &mut toml_edit::Table, pkg: &PkgInstallerLayer) {
    let pkg_table = installers_table
        .entry("pkg")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .expect("[dist.installers.pkg] should be a table");

    apply_installers_common(pkg_table, &pkg.common);

    apply_optional_value(
        pkg_table,
        "identifier",
        "# A unique identifier, in tld.domain.package format\n",
        pkg.identifier.clone(),
    );

    apply_optional_value(
        pkg_table,
        "install-location",
        "# The location to which software should be installed (defaults to /usr/local)\n",
        pkg.install_location.clone(),
    );

    // Finalize the table
    pkg_table
        .decor_mut()
        .set_prefix("\n# Configuration for the Mac .pkg installer\n");
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::LibraryStyle;
    use crate::init::apply_dist::InstallPathStrategy;
    use miette::IntoDiagnostic;
    use pretty_assertions::assert_eq;

    fn source() -> toml_edit::DocumentMut {
        let src = axoasset::SourceFile::new("fake-dist-workspace.toml", String::new());
        src.deserialize_toml_edit().into_diagnostic().unwrap()
    }

    // Given a DocumentMut, make sure it has a [dist] table, and return
    // a reference to that dist table.
    fn dist_table(doc: &mut toml_edit::DocumentMut) -> &mut toml_edit::Table {
        let dist = doc
            .entry("dist")
            .or_insert(Item::Table(Table::new()))
            .as_table_mut()
            .unwrap();
        // Don't show the empty top-level [dist].
        dist.set_implicit(true);
        // Return the table we just created.
        dist
    }

    #[test]
    fn apply_installers_empty() {
        let expected = "";

        let installers = Some(InstallerLayer {
            common: CommonInstallerLayer {
                install_path: None,
                install_success_msg: None,
                install_libraries: None,
                bin_aliases: None,
            },
            homebrew: None,
            msi: None,
            npm: None,
            powershell: None,
            shell: None,
            pkg: None,
            updater: None,
            always_use_latest_updater: None,
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &installers);

        let toml_text = table.to_string();
        assert_eq!(toml_text, expected);
    }

    #[test]
    fn apply_installers_everything_bools() {
        let expected = r#"
# Installer configuration for dist
[dist.installers]
# Path that installers should place binaries in
install-path = ["~/some-install-path/", "CARGO_HOME"]
# Custom message to display on successful install
install-success-msg = "default success message"
# Which kinds of packaged libraries to install
install-libraries = ["cdylib", "cstaticlib"]
# Whether to build a Homebrew installer
homebrew = true
# Whether to build an MSI installer
msi = true
# Whether to build an NPM installer
npm = true
# Whether to build a PowerShell installer
powershell = true
# Whether to build a Shell installer
shell = true
# Whether to build a Mac .pkg installer
pkg = true
# Whether to install an updater program alongside the software
updater = true
# Whether to always use the latest updater version instead of a fixed version
always-use-latest-updater = true
"#;

        let installers = Some(InstallerLayer {
            common: CommonInstallerLayer {
                install_path: Some(vec![
                    InstallPathStrategy::HomeSubdir {
                        subdir: "some-install-path/".to_string(),
                    },
                    InstallPathStrategy::CargoHome,
                ]),
                install_success_msg: Some("default success message".to_string()),
                install_libraries: Some(vec![LibraryStyle::CDynamic, LibraryStyle::CStatic]),
                bin_aliases: None,
            },
            homebrew: Some(BoolOr::Bool(true)),
            msi: Some(BoolOr::Bool(true)),
            npm: Some(BoolOr::Bool(true)),
            powershell: Some(BoolOr::Bool(true)),
            shell: Some(BoolOr::Bool(true)),
            pkg: Some(BoolOr::Bool(true)),
            updater: Some(true),
            always_use_latest_updater: Some(true),
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &installers);

        let toml_text = doc.to_string();
        assert_eq!(expected, toml_text);
    }

    #[test]
    fn apply_installers_complex() {
        let expected = r#"
# Installer configuration for dist
[dist.installers]
# Path that installers should place binaries in
install-path = ["~/some-install-path/", "CARGO_HOME"]
# Custom message to display on successful install
install-success-msg = "default success message"
# Which kinds of packaged libraries to install
install-libraries = ["cdylib", "cstaticlib"]
# Whether to build an MSI installer
msi = true
# Whether to build a PowerShell installer
powershell = true
# Whether to build a Shell installer
shell = true
# Whether to install an updater program alongside the software
updater = true
# Whether to always use the latest updater version instead of a fixed version
always-use-latest-updater = true

# Configuration for the Homebrew installer
[dist.installers.homebrew]
# A GitHub repo to push Homebrew formulas to
tap = "homebrew-tap"
# Customize the Homebrew formula name
formula = "homebrew-formula"

# Configuration for the NPM installer
[dist.installers.npm]
# The name of the npm package
package = "npm-package"
# The namespace to use when publishing this package to the npm registry
scope = "npm-scope"

# Configuration for the Mac .pkg installer
[dist.installers.pkg]
# A unique identifier, in tld.domain.package format
identifier = "pkg-identifier"
# The location to which software should be installed (defaults to /usr/local)
install-location = "pkg-install-location"
"#;

        let installers = Some(InstallerLayer {
            common: CommonInstallerLayer {
                install_path: Some(vec![
                    InstallPathStrategy::HomeSubdir {
                        subdir: "some-install-path/".to_string(),
                    },
                    InstallPathStrategy::CargoHome,
                ]),
                install_success_msg: Some("default success message".to_string()),
                install_libraries: Some(vec![LibraryStyle::CDynamic, LibraryStyle::CStatic]),
                bin_aliases: None,
            },
            homebrew: Some(BoolOr::Val(HomebrewInstallerLayer {
                common: CommonInstallerLayer::default(),
                tap: Some("homebrew-tap".to_string()),
                formula: Some("homebrew-formula".to_string()),
            })),
            msi: Some(BoolOr::Bool(true)),
            npm: Some(BoolOr::Val(NpmInstallerLayer {
                common: CommonInstallerLayer::default(),
                package: Some("npm-package".to_string()),
                scope: Some("npm-scope".to_string()),
            })),
            powershell: Some(BoolOr::Bool(true)),
            shell: Some(BoolOr::Bool(true)),
            pkg: Some(BoolOr::Val(PkgInstallerLayer {
                common: CommonInstallerLayer::default(),
                identifier: Some("pkg-identifier".to_string()),
                install_location: Some("pkg-install-location".to_string()),
            })),
            updater: Some(true),
            always_use_latest_updater: Some(true),
        });

        let mut doc = source();
        let table = dist_table(&mut doc);

        apply(table, &installers);

        let toml_text = doc.to_string();
        assert_eq!(expected, toml_text);
    }
}
