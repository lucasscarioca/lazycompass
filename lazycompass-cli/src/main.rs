use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use lazycompass_core::{
    AggregationRequest, AggregationTarget, OutputFormat, QueryRequest, QueryTarget,
};
use lazycompass_mongo::{AggregationSpec, Bson, Document, MongoExecutor, QuerySpec};
use lazycompass_storage::{ConfigPaths, StorageSnapshot, load_storage};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

const DEFAULT_INSTALL_URL: &str =
    "https://raw.githubusercontent.com/lucasscarioca/lazycompass/main/install.sh";

#[derive(Parser)]
#[command(name = "lazycompass")]
#[command(about = "MongoDB TUI + CLI client", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Query(QueryArgs),
    Agg(AggArgs),
    Upgrade(UpgradeArgs),
}

#[derive(Args)]
struct QueryArgs {
    name: Option<String>,

    #[arg(long)]
    connection: Option<String>,
    #[arg(long)]
    db: Option<String>,
    #[arg(long)]
    collection: Option<String>,
    #[arg(long)]
    filter: Option<String>,
    #[arg(long)]
    projection: Option<String>,
    #[arg(long)]
    sort: Option<String>,
    #[arg(long)]
    limit: Option<u64>,
    #[arg(long)]
    table: bool,
}

#[derive(Args)]
struct AggArgs {
    name: Option<String>,

    #[arg(long)]
    connection: Option<String>,
    #[arg(long)]
    db: Option<String>,
    #[arg(long)]
    collection: Option<String>,
    #[arg(long)]
    pipeline: Option<String>,
    #[arg(long)]
    table: bool,
}

#[derive(Args)]
struct UpgradeArgs {
    #[arg(long)]
    version: Option<String>,
    #[arg(long)]
    repo: Option<String>,
    #[arg(long)]
    from_source: bool,
    #[arg(long)]
    no_modify_path: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Query(args)) => run_query(args)?,
        Some(Commands::Agg(args)) => run_agg(args)?,
        Some(Commands::Upgrade(args)) => {
            run_upgrade(args)?;
        }
        None => {
            lazycompass_tui::run()?;
        }
    }

    Ok(())
}

fn build_query_request(args: QueryArgs) -> Result<QueryRequest> {
    let output = if args.table {
        OutputFormat::Table
    } else {
        OutputFormat::JsonPretty
    };

    if let Some(name) = &args.name {
        let mut conflicts = Vec::new();
        if args.db.is_some() {
            conflicts.push("--db");
        }
        if args.collection.is_some() {
            conflicts.push("--collection");
        }
        if args.filter.is_some() {
            conflicts.push("--filter");
        }
        if args.projection.is_some() {
            conflicts.push("--projection");
        }
        if args.sort.is_some() {
            conflicts.push("--sort");
        }
        if args.limit.is_some() {
            conflicts.push("--limit");
        }
        if !conflicts.is_empty() {
            anyhow::bail!(
                "saved query '{}' cannot be combined with {}",
                name,
                conflicts.join(", ")
            );
        }
    }

    let target = if let Some(name) = args.name {
        QueryTarget::Saved { name }
    } else {
        let database = args
            .db
            .ok_or_else(|| anyhow::anyhow!("--db is required for inline queries"))?;
        let collection = args
            .collection
            .ok_or_else(|| anyhow::anyhow!("--collection is required for inline queries"))?;

        QueryTarget::Inline {
            database,
            collection,
            filter: args.filter,
            projection: args.projection,
            sort: args.sort,
            limit: args.limit,
        }
    };

    Ok(QueryRequest {
        connection: args.connection,
        output,
        target,
    })
}

fn build_agg_request(args: AggArgs) -> Result<AggregationRequest> {
    let output = if args.table {
        OutputFormat::Table
    } else {
        OutputFormat::JsonPretty
    };

    if let Some(name) = &args.name {
        let mut conflicts = Vec::new();
        if args.db.is_some() {
            conflicts.push("--db");
        }
        if args.collection.is_some() {
            conflicts.push("--collection");
        }
        if args.pipeline.is_some() {
            conflicts.push("--pipeline");
        }
        if !conflicts.is_empty() {
            anyhow::bail!(
                "saved aggregation '{}' cannot be combined with {}",
                name,
                conflicts.join(", ")
            );
        }
    }

    let target = if let Some(name) = args.name {
        AggregationTarget::Saved { name }
    } else {
        let database = args
            .db
            .ok_or_else(|| anyhow::anyhow!("--db is required for inline aggregations"))?;
        let collection = args
            .collection
            .ok_or_else(|| anyhow::anyhow!("--collection is required for inline aggregations"))?;
        let pipeline = args
            .pipeline
            .ok_or_else(|| anyhow::anyhow!("--pipeline is required for inline aggregations"))?;

        AggregationTarget::Inline {
            database,
            collection,
            pipeline,
        }
    };

    Ok(AggregationRequest {
        connection: args.connection,
        output,
        target,
    })
}

fn run_query(args: QueryArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let request = build_query_request(args)?;
    let storage = load_storage(&paths)?;
    report_warnings(&storage);
    let spec = resolve_query_spec(&request, &storage)?;
    let executor = MongoExecutor::new();
    let runtime = tokio::runtime::Runtime::new().context("unable to start async runtime")?;
    let documents = runtime.block_on(executor.execute_query(&storage.config, &spec))?;
    print_documents(request.output, &documents)
}

fn run_agg(args: AggArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let request = build_agg_request(args)?;
    let storage = load_storage(&paths)?;
    report_warnings(&storage);
    let spec = resolve_aggregation_spec(&request, &storage)?;
    let executor = MongoExecutor::new();
    let runtime = tokio::runtime::Runtime::new().context("unable to start async runtime")?;
    let documents = runtime.block_on(executor.execute_aggregation(&storage.config, &spec))?;
    print_documents(request.output, &documents)
}

fn report_warnings(storage: &StorageSnapshot) {
    for warning in &storage.warnings {
        eprintln!("warning: {warning}");
    }
}

fn resolve_query_spec(request: &QueryRequest, storage: &StorageSnapshot) -> Result<QuerySpec> {
    match &request.target {
        QueryTarget::Saved { name } => {
            let saved = storage
                .queries
                .iter()
                .find(|query| query.name == *name)
                .with_context(|| format!("saved query '{name}' not found"))?;
            Ok(QuerySpec {
                connection: request
                    .connection
                    .clone()
                    .or_else(|| saved.connection.clone()),
                database: saved.database.clone(),
                collection: saved.collection.clone(),
                filter: saved.filter.clone(),
                projection: saved.projection.clone(),
                sort: saved.sort.clone(),
                limit: saved.limit,
            })
        }
        QueryTarget::Inline {
            database,
            collection,
            filter,
            projection,
            sort,
            limit,
        } => Ok(QuerySpec {
            connection: request.connection.clone(),
            database: database.clone(),
            collection: collection.clone(),
            filter: filter.clone(),
            projection: projection.clone(),
            sort: sort.clone(),
            limit: *limit,
        }),
    }
}

fn resolve_aggregation_spec(
    request: &AggregationRequest,
    storage: &StorageSnapshot,
) -> Result<AggregationSpec> {
    match &request.target {
        AggregationTarget::Saved { name } => {
            let saved = storage
                .aggregations
                .iter()
                .find(|aggregation| aggregation.name == *name)
                .with_context(|| format!("saved aggregation '{name}' not found"))?;
            Ok(AggregationSpec {
                connection: request
                    .connection
                    .clone()
                    .or_else(|| saved.connection.clone()),
                database: saved.database.clone(),
                collection: saved.collection.clone(),
                pipeline: saved.pipeline.clone(),
            })
        }
        AggregationTarget::Inline {
            database,
            collection,
            pipeline,
        } => Ok(AggregationSpec {
            connection: request.connection.clone(),
            database: database.clone(),
            collection: collection.clone(),
            pipeline: pipeline.clone(),
        }),
    }
}

fn print_documents(format: OutputFormat, documents: &[Document]) -> Result<()> {
    match format {
        OutputFormat::JsonPretty => {
            let output = serde_json::to_string_pretty(documents)
                .context("unable to serialize results as JSON")?;
            println!("{output}");
        }
        OutputFormat::Table => {
            let output = format_table(documents);
            println!("{output}");
        }
    }
    Ok(())
}

fn format_table(documents: &[Document]) -> String {
    if documents.is_empty() {
        return "no results".to_string();
    }

    let mut columns = BTreeSet::new();
    for document in documents {
        for (key, value) in document.iter() {
            if is_scalar(value) {
                columns.insert(key.to_string());
            }
        }
    }

    if columns.is_empty() {
        return "no scalar fields to display".to_string();
    }

    let columns: Vec<String> = columns.into_iter().collect();
    let mut rows = Vec::with_capacity(documents.len());
    for document in documents {
        let mut row = Vec::with_capacity(columns.len());
        for column in &columns {
            let cell = match document.get(column) {
                Some(value) if is_scalar(value) => format_scalar(value),
                _ => String::new(),
            };
            row.push(cell);
        }
        rows.push(row);
    }

    let widths = column_widths(&columns, &rows);
    let mut output = String::new();
    output.push_str(&format_row(&columns, &widths));
    output.push('\n');
    output.push_str(&format_separator(&widths));
    for row in rows {
        output.push('\n');
        output.push_str(&format_row(&row, &widths));
    }
    output
}

fn column_widths(headers: &[String], rows: &[Vec<String>]) -> Vec<usize> {
    let mut widths: Vec<usize> = headers.iter().map(|header| header.len()).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            if cell.len() > widths[index] {
                widths[index] = cell.len();
            }
        }
    }
    widths
}

fn format_row(cells: &[String], widths: &[usize]) -> String {
    let mut row = String::new();
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            row.push_str(" | ");
        }
        let width = widths[index];
        row.push_str(&format!("{cell:width$}", width = width));
    }
    row
}

fn format_separator(widths: &[usize]) -> String {
    let mut line = String::new();
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            line.push_str("-+-");
        }
        line.push_str(&"-".repeat(*width));
    }
    line
}

fn is_scalar(value: &Bson) -> bool {
    !matches!(value, Bson::Document(_) | Bson::Array(_))
}

fn format_scalar(value: &Bson) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(value)) => value,
        Ok(serde_json::Value::Null) => "null".to_string(),
        Ok(value) => value.to_string(),
        Err(_) => format!("{value:?}"),
    }
}

fn run_upgrade(args: UpgradeArgs) -> Result<()> {
    let mut installer_args = Vec::new();
    if let Some(version) = args.version {
        installer_args.push("--version".to_string());
        installer_args.push(version);
    }
    if let Some(repo) = args.repo {
        installer_args.push("--repo".to_string());
        installer_args.push(repo);
    }
    if args.from_source {
        installer_args.push("--from-source".to_string());
    }
    if args.no_modify_path {
        installer_args.push("--no-modify-path".to_string());
    }

    if Path::new("install.sh").is_file() {
        let status = Command::new("bash")
            .arg("install.sh")
            .args(&installer_args)
            .status()
            .context("failed to run install.sh")?;
        if !status.success() {
            anyhow::bail!("install.sh exited with non-zero status");
        }
        return Ok(());
    }

    let url = std::env::var("LAZYCOMPASS_INSTALL_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_INSTALL_URL.to_string());
    let status = Command::new("bash")
        .arg("-c")
        .arg("curl -fsSL \"$1\" | bash -s -- \"${@:2}\"")
        .arg("bash")
        .arg(url)
        .args(&installer_args)
        .status()
        .context("failed to run installer from URL")?;
    if !status.success() {
        anyhow::bail!("installer exited with non-zero status");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query_request_rejects_inline_with_name() {
        let args = QueryArgs {
            name: Some("saved".to_string()),
            connection: None,
            db: Some("lazycompass".to_string()),
            collection: None,
            filter: None,
            projection: None,
            sort: None,
            limit: None,
            table: false,
        };

        assert!(build_query_request(args).is_err());
    }

    #[test]
    fn build_agg_request_rejects_inline_with_name() {
        let args = AggArgs {
            name: Some("saved".to_string()),
            connection: None,
            db: None,
            collection: Some("orders".to_string()),
            pipeline: None,
            table: false,
        };

        assert!(build_agg_request(args).is_err());
    }
}
