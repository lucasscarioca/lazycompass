use anyhow::{Context, Result};
use clap::Parser;
use lazycompass_storage::{ConfigPaths, load_config};

use crate::cli::{Cli, Commands};
use crate::commands::{
    run_agg, run_config, run_init, run_insert, run_query, run_update, run_upgrade,
};
use crate::logging::{apply_cli_overrides, init_logging};

pub(crate) fn run() -> Result<()> {
    let action = dispatch(Cli::parse());
    execute(action)
}

enum AppAction {
    Init(crate::cli::InitArgs),
    Query {
        args: crate::cli::QueryArgs,
        write_enabled: bool,
        allow_pipeline_writes: bool,
        allow_insecure: bool,
    },
    Agg {
        args: crate::cli::AggArgs,
        write_enabled: bool,
        allow_pipeline_writes: bool,
        allow_insecure: bool,
    },
    Insert {
        args: crate::cli::InsertArgs,
        write_enabled: bool,
        allow_pipeline_writes: bool,
        allow_insecure: bool,
    },
    Update {
        args: crate::cli::UpdateArgs,
        write_enabled: bool,
        allow_pipeline_writes: bool,
        allow_insecure: bool,
    },
    Config(crate::cli::ConfigArgs),
    Upgrade(crate::cli::UpgradeArgs),
    Tui {
        write_enabled: bool,
        allow_pipeline_writes: bool,
        allow_insecure: bool,
    },
}

fn dispatch(cli: Cli) -> AppAction {
    match cli.command {
        Some(Commands::Init(args)) => AppAction::Init(args),
        Some(Commands::Query(args)) => AppAction::Query {
            args,
            write_enabled: cli.write_enabled,
            allow_pipeline_writes: cli.allow_pipeline_writes,
            allow_insecure: cli.allow_insecure,
        },
        Some(Commands::Agg(args)) => AppAction::Agg {
            args,
            write_enabled: cli.write_enabled,
            allow_pipeline_writes: cli.allow_pipeline_writes,
            allow_insecure: cli.allow_insecure,
        },
        Some(Commands::Insert(args)) => AppAction::Insert {
            args,
            write_enabled: cli.write_enabled,
            allow_pipeline_writes: cli.allow_pipeline_writes,
            allow_insecure: cli.allow_insecure,
        },
        Some(Commands::Update(args)) => AppAction::Update {
            args,
            write_enabled: cli.write_enabled,
            allow_pipeline_writes: cli.allow_pipeline_writes,
            allow_insecure: cli.allow_insecure,
        },
        Some(Commands::Config(args)) => AppAction::Config(args),
        Some(Commands::Upgrade(args)) => AppAction::Upgrade(args),
        None => AppAction::Tui {
            write_enabled: cli.write_enabled,
            allow_pipeline_writes: cli.allow_pipeline_writes,
            allow_insecure: cli.allow_insecure,
        },
    }
}

fn execute(action: AppAction) -> Result<()> {
    match action {
        AppAction::Init(args) => {
            run_init(args)?;
        }
        AppAction::Query {
            args,
            write_enabled,
            allow_pipeline_writes,
            allow_insecure,
        } => run_query(args, write_enabled, allow_pipeline_writes, allow_insecure)?,
        AppAction::Agg {
            args,
            write_enabled,
            allow_pipeline_writes,
            allow_insecure,
        } => run_agg(args, write_enabled, allow_pipeline_writes, allow_insecure)?,
        AppAction::Insert {
            args,
            write_enabled,
            allow_pipeline_writes,
            allow_insecure,
        } => run_insert(args, write_enabled, allow_pipeline_writes, allow_insecure)?,
        AppAction::Update {
            args,
            write_enabled,
            allow_pipeline_writes,
            allow_insecure,
        } => run_update(args, write_enabled, allow_pipeline_writes, allow_insecure)?,
        AppAction::Config(args) => {
            run_config(args)?;
        }
        AppAction::Upgrade(args) => {
            run_upgrade(args)?;
        }
        AppAction::Tui {
            write_enabled,
            allow_pipeline_writes,
            allow_insecure,
        } => {
            let cwd = std::env::current_dir().context("unable to resolve current directory")?;
            let paths = ConfigPaths::resolve_from(&cwd)?;
            let mut config = load_config(&paths)?;
            apply_cli_overrides(
                &mut config,
                write_enabled,
                allow_pipeline_writes,
                allow_insecure,
            );
            init_logging(&paths, &config)?;
            tracing::info!(component = "tui", command = "tui", "lazycompass started");
            lazycompass_tui::run(config)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{AppAction, dispatch};
    use crate::cli::{Cli, Commands};

    #[test]
    fn dispatch_routes_to_tui_when_no_subcommand() {
        let cli = Cli::parse_from(["lazycompass", "--write-enabled"]);
        let action = dispatch(cli);
        assert!(matches!(
            action,
            AppAction::Tui {
                write_enabled: true,
                allow_pipeline_writes: false,
                allow_insecure: false,
            }
        ));
    }

    #[test]
    fn dispatch_routes_insert_with_global_flags() {
        let cli = Cli::parse_from([
            "lazycompass",
            "--write-enabled",
            "--allow-pipeline-writes",
            "insert",
            "--collection",
            "users",
            "--document",
            "{}",
        ]);
        let action = dispatch(cli);
        assert!(matches!(
            action,
            AppAction::Insert {
                write_enabled: true,
                allow_pipeline_writes: true,
                allow_insecure: false,
                ..
            }
        ));
    }

    #[test]
    fn dispatch_routes_upgrade() {
        let cli = Cli::parse_from(["lazycompass", "upgrade", "--version", "1.2.3"]);
        let action = dispatch(cli);
        assert!(matches!(action, AppAction::Upgrade(_)));
    }

    #[test]
    fn cli_parser_accepts_query_output_flag() {
        let cli = Cli::parse_from([
            "lazycompass",
            "query",
            "recent_orders",
            "--db",
            "app",
            "--collection",
            "orders",
            "-o",
            "results.json",
        ]);

        match cli.command {
            Some(Commands::Query(args)) => {
                assert_eq!(args.name.as_deref(), Some("recent_orders"));
                assert_eq!(
                    args.output.as_deref(),
                    Some(std::path::Path::new("results.json"))
                );
            }
            _ => panic!("expected query command"),
        }
    }
}
