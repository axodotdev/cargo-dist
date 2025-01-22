pub mod console_helpers;
mod init_args;
pub(crate) mod v0;

pub use v0::do_init as do_init_v0;

use crate::{
    config::{self, v1::TomlLayer},
    do_generate, do_migrate,
    errors::DistResult,
    migrate,
    tasks::SortedMap,
    GenerateArgs,
};

use axoasset::toml_edit;
use axoproject::WorkspaceKind;
use console_helpers::theme;
pub use init_args::InitArgs;
use serde::{Deserialize, Serialize};

mod dist_metadata;
use dist_metadata::get_new_dist_metadata;

mod apply_dist_metadata;
use apply_dist_metadata::apply_dist_to_metadata;

mod profile;
use profile::init_dist_profile;

#[derive(Default, Deserialize, Serialize)]
struct V1MultiDistMetadata {
    /// `[workspace.metadata.dist]`
    workspace: Option<TomlLayer>,
    /// package_name => `[package.metadata.dist]`
    #[serde(default)]
    packages: SortedMap<String, TomlLayer>,
}

/// Run 'dist init'
pub fn do_init(cfg: &config::Config, args: &InitArgs) -> DistResult<()> {
    if !config::version::want_v1()? {
        return do_init_v0(cfg, args);
    }

    eprintln!("!!!! DIST_V1 was set, so we're using the v1 config and related code !!!");
    eprintln!("!!!! This is under heavy development -- here be dragons, you may be eaten by a grue, etc !!!");

    let ctrlc_handler = console_helpers::ctrlc_handler();
    let check = console::style("✔".to_string()).for_stderr().green();

    unimplemented!();

    eprintln!("{check} dist is setup!");
    eprintln!();

    // regenerate anything that needs to be
    if !args.no_generate {
        eprintln!("running 'dist generate' to apply any changes");
        eprintln!();

        let ci_args = GenerateArgs {
            check: false,
            modes: vec![],
        };
        do_generate(cfg, &ci_args)?;
    }
    Ok(())
}

/// Update a workspace toml-edit document with the current DistMetadata value
pub(crate) fn apply_dist_to_workspace_toml(
    workspace_toml: &mut toml_edit::DocumentMut,
    meta: &TomlLayer,
) {
    let metadata = workspace_toml.as_item_mut();
    apply_dist_to_metadata(metadata, meta);
}
