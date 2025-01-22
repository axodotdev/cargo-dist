use crate::{config::v1::TomlLayer, config::Config, errors::DistResult, init::InitArgs};
use axoproject::WorkspaceGraph;

/// Initialize [workspace.metadata.dist] with default values based on what was passed on the CLI
///
/// Returns whether the initialization was actually done
/// and whether ci was set
pub fn get_new_dist_metadata(
    cfg: &Config,
    args: &InitArgs,
    workspaces: &WorkspaceGraph,
) -> DistResult<TomlLayer> {
    unimplemented!()
}
