use anyhow::{Context, Result};
use lazycompass_storage::ConfigPaths;

use crate::cli::InitArgs;

use super::config::{resolve_config_scope, run_config_add_connection};

pub(crate) fn run_init(args: InitArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let scope = resolve_config_scope(&paths, args.global, args.repo);
    run_config_add_connection(&paths, scope)
}
