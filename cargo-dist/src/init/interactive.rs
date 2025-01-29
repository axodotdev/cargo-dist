use axoproject::WorkspaceGraph;
use crate::{config, migrate};
use crate::config::v1::TomlLayer;
//use crate::config::{CiStyle, InstallerStyle, PublishStyle};
use crate::config::Config;
use crate::errors::{DistError, DistResult};
/*use crate::platform::triple_to_display_name;
use dialoguer::{Confirm, Input, MultiSelect};
use dist_schema::TripleNameRef;
use semver::Version;*/
use super::console_helpers;
use super::InitArgs;

/// Initialize [dist] with values based on what was passed on the CLI
pub fn get_new_metadata(
    cfg: &Config,
    args: &InitArgs,
    workspaces: &WorkspaceGraph,
) -> DistResult<TomlLayer> {
    let root_workspace = workspaces.root_workspace();
    let has_config = migrate::has_metadata_table(root_workspace);

    let mut meta = if has_config {
        config::v1::load_dist(&root_workspace.manifest_path)?
    } else {
        TomlLayer::default()
    };

    /*
    // Clone this to simplify checking for settings changes
    let orig_meta = meta.clone();

    // Now prompt the user interactively to initialize these...

    // Tune the theming a bit
    let theme = console_helpers::theme();
    // Some indicators we'll use in a few places
    let check = console_helpers::checkmark();
    let notice = console_helpers::notice();
    */

    // TODO(migration): implement interactive prompts

    Ok(meta)
}
