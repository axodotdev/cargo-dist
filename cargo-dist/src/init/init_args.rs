use crate::config::HostingStyle;
use camino::Utf8PathBuf;

/// Arguments for `dist init` ([`do_init`][])
#[derive(Debug)]
pub struct InitArgs {
    /// Whether to auto-accept the default values for interactive prompts
    pub yes: bool,
    /// Don't automatically generate ci
    pub no_generate: bool,
    /// A path to a json file containing values to set in workspace.metadata.dist
    pub with_json_config: Option<Utf8PathBuf>,
    /// Hosts to enable
    pub host: Vec<HostingStyle>,
}
