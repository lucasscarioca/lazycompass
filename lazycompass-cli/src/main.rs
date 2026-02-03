use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use lazycompass_core::{
    AggregationRequest, AggregationTarget, OutputFormat, QueryRequest, QueryTarget,
};
use lazycompass_storage::ConfigPaths;

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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Query(args)) => {
            let cwd = std::env::current_dir().context("unable to resolve current directory")?;
            let paths = ConfigPaths::resolve_from(&cwd)?;
            let request = build_query_request(args)?;
            print_query_summary(&request, &paths);
        }
        Some(Commands::Agg(args)) => {
            let cwd = std::env::current_dir().context("unable to resolve current directory")?;
            let paths = ConfigPaths::resolve_from(&cwd)?;
            let request = build_agg_request(args)?;
            print_agg_summary(&request, &paths);
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

fn print_query_summary(request: &QueryRequest, paths: &ConfigPaths) {
    println!("lazycompass-cli (stub)");
    println!("- mode: query");
    println!("- output: {}", request.output.label());
    match &request.target {
        QueryTarget::Saved { name } => {
            println!("- target: saved query '{}'", name);
        }
        QueryTarget::Inline {
            database,
            collection,
            filter,
            projection,
            sort,
            limit,
        } => {
            println!("- target: inline {}.{}", database, collection);
            if let Some(filter) = filter {
                println!("- filter: {}", filter);
            }
            if let Some(projection) = projection {
                println!("- projection: {}", projection);
            }
            if let Some(sort) = sort {
                println!("- sort: {}", sort);
            }
            if let Some(limit) = limit {
                println!("- limit: {}", limit);
            }
        }
    }
    print_path_summary(paths);
}

fn print_agg_summary(request: &AggregationRequest, paths: &ConfigPaths) {
    println!("lazycompass-cli (stub)");
    println!("- mode: aggregation");
    println!("- output: {}", request.output.label());
    match &request.target {
        AggregationTarget::Saved { name } => {
            println!("- target: saved aggregation '{}'", name);
        }
        AggregationTarget::Inline {
            database,
            collection,
            pipeline,
        } => {
            println!("- target: inline {}.{}", database, collection);
            println!("- pipeline: {}", pipeline);
        }
    }
    print_path_summary(paths);
}

fn print_path_summary(paths: &ConfigPaths) {
    println!("- global config: {}", paths.global_config_path().display());
    match paths.repo_config_path() {
        Some(path) => println!("- repo config: {}", path.display()),
        None => println!("- repo config: (none)"),
    }
}
