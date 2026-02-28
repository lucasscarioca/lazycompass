use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lazycompass")]
#[command(about = "MongoDB TUI + CLI client", version)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Commands>,
    #[arg(long, global = true)]
    pub(crate) write_enabled: bool,
    #[arg(long, global = true)]
    pub(crate) allow_pipeline_writes: bool,
    #[arg(long, global = true)]
    pub(crate) allow_insecure: bool,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    Init(InitArgs),
    Query(QueryArgs),
    Agg(AggArgs),
    Insert(InsertArgs),
    Update(UpdateArgs),
    Config(ConfigArgs),
    Upgrade(UpgradeArgs),
}

#[derive(Args)]
pub(crate) struct QueryArgs {
    pub(crate) name: Option<String>,

    #[arg(long)]
    pub(crate) connection: Option<String>,
    #[arg(long)]
    pub(crate) db: Option<String>,
    #[arg(long)]
    pub(crate) collection: Option<String>,
    #[arg(long)]
    pub(crate) filter: Option<String>,
    #[arg(long)]
    pub(crate) projection: Option<String>,
    #[arg(long)]
    pub(crate) sort: Option<String>,
    #[arg(long)]
    pub(crate) limit: Option<u64>,
    #[arg(long)]
    pub(crate) table: bool,
    #[arg(short = 'o', long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct AggArgs {
    pub(crate) name: Option<String>,

    #[arg(long)]
    pub(crate) connection: Option<String>,
    #[arg(long)]
    pub(crate) db: Option<String>,
    #[arg(long)]
    pub(crate) collection: Option<String>,
    #[arg(long)]
    pub(crate) pipeline: Option<String>,
    #[arg(long)]
    pub(crate) table: bool,
    #[arg(short = 'o', long)]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args)]
pub(crate) struct InsertArgs {
    #[arg(long)]
    pub(crate) connection: Option<String>,
    #[arg(long)]
    pub(crate) db: Option<String>,
    #[arg(long)]
    pub(crate) collection: Option<String>,
    /// JSON document as string
    #[arg(long)]
    pub(crate) document: Option<String>,
    /// Path to JSON file containing document
    #[arg(long)]
    pub(crate) file: Option<String>,
}

#[derive(Args)]
pub(crate) struct UpdateArgs {
    #[arg(long)]
    pub(crate) connection: Option<String>,
    #[arg(long)]
    pub(crate) db: Option<String>,
    #[arg(long)]
    pub(crate) collection: Option<String>,
    /// Document ID to update (JSON format, e.g., '"id"' or '{"$oid":"..."}')
    #[arg(long)]
    pub(crate) id: String,
    /// JSON document as string
    #[arg(long)]
    pub(crate) document: Option<String>,
    /// Path to JSON file containing document
    #[arg(long)]
    pub(crate) file: Option<String>,
}

#[derive(Args)]
pub(crate) struct ConfigArgs {
    #[command(subcommand)]
    pub(crate) command: ConfigCommands,

    /// Use global config instead of repo config
    #[arg(long, global = true, group = "scope")]
    pub(crate) global: bool,

    /// Use repo config instead of global config
    #[arg(long, global = true, group = "scope")]
    pub(crate) repo: bool,
}

#[derive(Args)]
pub(crate) struct InitArgs {
    /// Use global config instead of repo config
    #[arg(long, group = "scope")]
    pub(crate) global: bool,

    /// Use repo config instead of global config
    #[arg(long, group = "scope")]
    pub(crate) repo: bool,
}

#[derive(Subcommand)]
pub(crate) enum ConfigCommands {
    /// Open the config file in your default editor
    Edit,
    /// Add a new connection via interactive editor
    AddConnection,
}

#[derive(Args)]
pub(crate) struct UpgradeArgs {
    #[arg(long)]
    pub(crate) version: Option<String>,
    #[arg(long)]
    pub(crate) repo: Option<String>,
    #[arg(long)]
    pub(crate) from_source: bool,
    #[arg(long)]
    pub(crate) no_modify_path: bool,
}
