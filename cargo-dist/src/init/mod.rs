pub(crate) mod v0;
pub use v0::do_init;
mod apply_dist;
pub mod console_helpers;
mod dist_profile;
mod init_args;

/*
use crate::config::{self, v1::TomlLayer, Config};
use crate::errors::DistResult;
use crate::migrate;
use crate::SortedMap;
use crate::{do_generate, GenerateArgs};
use console_helpers::theme;
*/
pub use dist_profile::init_dist_profile;
pub use init_args::InitArgs;
