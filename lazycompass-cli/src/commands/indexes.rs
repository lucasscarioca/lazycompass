use anyhow::{Context, Result};
use lazycompass_core::OutputFormat;
use lazycompass_mongo::MongoExecutor;
use lazycompass_storage::{ConfigPaths, load_storage};

use super::database::resolve_database_arg;
use crate::cli::IndexesArgs;
use crate::errors::report_warnings;
use crate::logging::{apply_cli_overrides, init_logging};
use crate::output::print_documents;

pub(crate) fn run_indexes(
    args: IndexesArgs,
    _dangerously_enable_write: bool,
    _allow_pipeline_writes: bool,
    allow_insecure: bool,
) -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let storage = load_storage(&paths)?;
    let mut config = storage.config.clone();
    apply_cli_overrides(&mut config, allow_insecure);
    init_logging(&paths, &config)?;
    tracing::info!(
        component = "cli",
        command = "indexes",
        "lazycompass started"
    );
    report_warnings(&storage);

    let output = output_format(&args);
    let database = resolve_database_arg(
        &config,
        args.connection.as_deref(),
        args.db,
        "--db is required for index listing",
    )?;
    let collection = args
        .collection
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("--collection is required"))?;
    let output_path = args.output.clone();

    let executor = MongoExecutor::new();
    let connection = executor.resolve_connection(&config, args.connection.as_deref())?;
    tracing::info!(
        component = "cli",
        command = "indexes",
        connection = connection.name.as_str(),
        database = database.as_str(),
        collection = collection.as_str(),
        "listing indexes"
    );

    let runtime = tokio::runtime::Runtime::new().context("unable to start async runtime")?;
    let documents = runtime.block_on(executor.list_indexes(
        &config,
        Some(&connection.name),
        &database,
        &collection,
    ))?;
    print_documents(output, &documents, output_path.as_deref())
}

fn output_format(args: &IndexesArgs) -> OutputFormat {
    if args.csv {
        OutputFormat::Csv
    } else if args.table {
        OutputFormat::Table
    } else {
        OutputFormat::JsonPretty
    }
}

#[cfg(test)]
mod tests {
    use lazycompass_core::{Config, ConnectionSpec, OutputFormat};

    use super::output_format;
    use crate::cli::IndexesArgs;
    use crate::commands::database::resolve_database_arg;

    fn base_args() -> IndexesArgs {
        IndexesArgs {
            connection: None,
            db: Some("app".to_string()),
            collection: Some("users".to_string()),
            table: false,
            csv: false,
            output: None,
        }
    }

    fn config_with_connection(name: &str, default_database: Option<&str>) -> Config {
        Config {
            connections: vec![ConnectionSpec {
                name: name.to_string(),
                uri: format!("mongodb://{name}:27017"),
                default_database: default_database.map(ToString::to_string),
            }],
            ..Config::default()
        }
    }

    #[test]
    fn output_format_defaults_to_json_pretty() {
        assert!(matches!(
            output_format(&base_args()),
            OutputFormat::JsonPretty
        ));
    }

    #[test]
    fn output_format_uses_table_when_requested() {
        let mut args = base_args();
        args.table = true;
        assert!(matches!(output_format(&args), OutputFormat::Table));
    }

    #[test]
    fn output_format_uses_csv_when_requested() {
        let mut args = base_args();
        args.csv = true;
        assert!(matches!(output_format(&args), OutputFormat::Csv));
    }

    #[test]
    fn indexes_command_uses_default_database_resolution() {
        let config = config_with_connection("local", Some("default"));
        let database = resolve_database_arg(&config, Some("local"), None, "--db is required")
            .expect("resolve database");
        assert_eq!(database, "default");
    }

    #[test]
    fn indexes_command_requires_collection() {
        let args = IndexesArgs {
            collection: None,
            ..base_args()
        };

        let error = args
            .collection
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("--collection is required"))
            .expect_err("missing collection");
        assert_eq!(error.to_string(), "--collection is required");
    }
}
