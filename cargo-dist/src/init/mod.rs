pub(crate) mod v0;
pub use v0::do_init;
mod apply_dist;
pub mod console_helpers;
mod dist_profile;
mod init_args;

use console_helpers::theme;
use crate::{do_generate, GenerateArgs};
use crate::SortedMap;
use crate::config::{self, Config, v1::TomlLayer};
use crate::errors::DistResult;
use crate::migrate;
pub use dist_profile::init_dist_profile;
pub use init_args::InitArgs;
