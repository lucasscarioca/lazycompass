use anyhow::{Context, Result};
use clap::Parser;
use lazycompass_storage::{ConfigPaths, load_config};

use crate::cli::{Cli, Commands};
use crate::commands::{
    run_agg, run_config, run_init, run_insert, run_query, run_update, run_upgrade,
};
use crate::logging::{apply_cli_overrides, init_logging};

pub(crate) fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init(args)) => {
            run_init(args)?;
        }
        Some(Commands::Query(args)) => run_query(
            args,
            cli.write_enabled,
            cli.allow_pipeline_writes,
            cli.allow_insecure,
        )?,
        Some(Commands::Agg(args)) => run_agg(
            args,
            cli.write_enabled,
            cli.allow_pipeline_writes,
            cli.allow_insecure,
        )?,
        Some(Commands::Insert(args)) => run_insert(
            args,
            cli.write_enabled,
            cli.allow_pipeline_writes,
            cli.allow_insecure,
        )?,
        Some(Commands::Update(args)) => run_update(
            args,
            cli.write_enabled,
            cli.allow_pipeline_writes,
            cli.allow_insecure,
        )?,
        Some(Commands::Config(args)) => {
            run_config(args)?;
        }
        Some(Commands::Upgrade(args)) => {
            run_upgrade(args)?;
        }
        None => {
            let cwd = std::env::current_dir().context("unable to resolve current directory")?;
            let paths = ConfigPaths::resolve_from(&cwd)?;
            let mut config = load_config(&paths)?;
            apply_cli_overrides(
                &mut config,
                cli.write_enabled,
                cli.allow_pipeline_writes,
                cli.allow_insecure,
            );
            init_logging(&paths, &config)?;
            tracing::info!(component = "tui", command = "tui", "lazycompass started");
            lazycompass_tui::run(config)?;
        }
    }

    Ok(())
}
