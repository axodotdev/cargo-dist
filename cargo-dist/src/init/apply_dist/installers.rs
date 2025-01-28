use axoasset::toml_edit;
use crate::config::v1::layer::{BoolOr, BoolOrOptExt};
use super::helpers::*;

use crate::config::v1::installers::{
    homebrew::HomebrewInstallerLayer, msi::MsiInstallerLayer, npm::NpmInstallerLayer,
    pkg::PkgInstallerLayer, powershell::PowershellInstallerLayer,
    shell::ShellInstallerLayer, CommonInstallerLayer, InstallerLayer,
};

pub fn apply(table: &mut toml_edit::Table, installers: &Option<InstallerLayer>) {
    let Some(installers) = installers else {
        return;
    };
    let Some(installers_table) = table.get_mut("installers") else {
        return;
    };
    let toml_edit::Item::Table(installers_table) = installers_table else {
        panic!("Expected [dist.installers] to be a table");
    };

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
                    "\n# Configuration for the Mac .pkg installer\n",
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

    // / Aliases to install binaries as
    // TODO(migration): handle `pub bin_aliases: Option<SortedMap<String, Vec<String>>>`
}

fn apply_installers_homebrew(
    installers_table: &mut toml_edit::Table,
    homebrew: &HomebrewInstallerLayer,
) {
    let Some(homebrew_table) = installers_table.get_mut("homebrew") else {
        return;
    };
    let toml_edit::Item::Table(homebrew_table) = homebrew_table else {
        panic!("Expected [dist.installers.homebrew] to be a table");
    };

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
        .set_prefix("\n# Configure the built Homebrew installer\n");
}

fn apply_installers_msi(installers_table: &mut toml_edit::Table, msi: &MsiInstallerLayer) {
    let Some(msi_table) = installers_table.get_mut("msi") else {
        return;
    };
    let toml_edit::Item::Table(msi_table) = msi_table else {
        panic!("Expected [dist.installers.msi] to be a table");
    };

    apply_installers_common(msi_table, &msi.common);

    // There are no items under MsiInstallerConfig aside from `msi.common`.

    msi_table
        .decor_mut()
        .set_prefix("\n# Configure the built MSI installer\n");
}

fn apply_installers_npm(installers_table: &mut toml_edit::Table, npm: &NpmInstallerLayer) {
    let Some(npm_table) = installers_table.get_mut("npm") else {
        return;
    };
    let toml_edit::Item::Table(npm_table) = npm_table else {
        panic!("Expected [dist.installers.npm] to be a table");
    };

    apply_installers_common(npm_table, &npm.common);

    apply_optional_value(
        npm_table,
        "package",
        "# The npm package should have this name\n",
        npm.package.as_deref(),
    );

    apply_optional_value(
        npm_table,
        "scope",
        "# A namespace to use when publishing this package to the npm registry\n",
        npm.scope.as_deref(),
    );
}

fn apply_installers_powershell(
    installers_table: &mut toml_edit::Table,
    powershell: &PowershellInstallerLayer,
) {
    let Some(powershell_table) = installers_table.get_mut("powershell") else {
        return;
    };
    let toml_edit::Item::Table(powershell_table) = powershell_table else {
        panic!("Expected [dist.installers.powershell] to be a table");
    };

    apply_installers_common(powershell_table, &powershell.common);

    // TODO(migration): implement this (similar to shell)
}

fn apply_installers_shell(installers_table: &mut toml_edit::Table, shell: &ShellInstallerLayer) {
    let Some(shell_table) = installers_table.get_mut("shell") else {
        return;
    };
    let toml_edit::Item::Table(shell_table) = shell_table else {
        panic!("Expected [dist.installers.shell] to be a table");
    };

    apply_installers_common(shell_table, &shell.common);

    // TODO(migration): implement this
}

fn apply_installers_pkg(installers_table: &mut toml_edit::Table, pkg: &PkgInstallerLayer) {
    let Some(pkg_table) = installers_table.get_mut("pkg") else {
        return;
    };
    let toml_edit::Item::Table(pkg_table) = pkg_table else {
        panic!("Expected [dist.installers.pkg] to be a table");
    };

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
