use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lazycompass")]
#[command(
    about = "Vim-first MongoDB TUI and CLI",
    long_about = "Run LazyCompass without a subcommand to open the TUI. Use subcommands for saved specs, inline queries, config management, index inspection, and write operations.",
    after_help = "Examples:\n  lazycompass\n  lazycompass query app.users.active_users\n  lazycompass query --collection users --filter '{\"active\":true}'\n  lazycompass agg --collection orders --pipeline '[]'\n  lazycompass --dangerously-enable-write insert --collection users --document '{\"email\":\"a@example.com\"}'",
    version
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Commands>,
    #[arg(
        long = "dangerously-enable-write",
        visible_alias = "yolo",
        global = true,
        help = "Enable document writes for this run"
    )]
    pub(crate) dangerously_enable_write: bool,
    #[arg(
        long,
        global = true,
        help = "Allow $out/$merge in aggregations for this run",
        requires = "dangerously_enable_write"
    )]
    pub(crate) allow_pipeline_writes: bool,
    #[arg(
        long,
        global = true,
        help = "Permit insecure Mongo connections for this run"
    )]
    pub(crate) allow_insecure: bool,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    #[command(about = "Bootstrap config and add a first connection")]
    Init(InitArgs),
    #[command(about = "List indexes for a collection")]
    Indexes(IndexesArgs),
    #[command(about = "Run a saved query or an inline find query")]
    Query(QueryArgs),
    #[command(about = "Run a saved aggregation or an inline pipeline")]
    Agg(AggArgs),
    #[command(about = "Insert one document into a collection")]
    Insert(InsertArgs),
    #[command(about = "Replace one document by _id")]
    Update(UpdateArgs),
    #[command(about = "Open or update LazyCompass config")]
    Config(ConfigArgs),
    #[command(about = "Upgrade LazyCompass from release assets or source")]
    Upgrade(UpgradeArgs),
}

#[derive(Args)]
#[command(
    about = "List indexes for a collection",
    long_about = "List indexes for a collection and render them as pretty JSON, CSV, or a table."
)]
pub(crate) struct IndexesArgs {
    #[arg(long, help = "Connection name from config")]
    pub(crate) connection: Option<String>,
    #[arg(
        long,
        help = "Database name; falls back to the connection default_database"
    )]
    pub(crate) db: Option<String>,
    #[arg(long, help = "Collection name to inspect")]
    pub(crate) collection: Option<String>,
    #[arg(long, help = "Render output as a table")]
    #[arg(conflicts_with = "csv")]
    pub(crate) table: bool,
    #[arg(long, help = "Render output as CSV")]
    #[arg(conflicts_with = "table")]
    pub(crate) csv: bool,
    #[arg(short = 'o', long, help = "Write rendered output to a file")]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args)]
#[command(
    about = "Run a saved query or an inline find query",
    long_about = "Run a saved query by ID, or run an inline find query with --db/--collection and optional filter, projection, sort, and limit. Shared saved queries can use the selected connection default_database when --db is omitted."
)]
pub(crate) struct QueryArgs {
    #[arg(help = "Saved query ID; omit to run an inline query")]
    pub(crate) name: Option<String>,

    #[arg(long, help = "Connection name from config")]
    pub(crate) connection: Option<String>,
    #[arg(
        long,
        help = "Database name; falls back to the connection default_database"
    )]
    pub(crate) db: Option<String>,
    #[arg(long, help = "Collection name")]
    pub(crate) collection: Option<String>,
    #[arg(long, help = "Inline Mongo find filter as JSON")]
    pub(crate) filter: Option<String>,
    #[arg(long, help = "Inline Mongo projection as JSON")]
    pub(crate) projection: Option<String>,
    #[arg(long, help = "Inline Mongo sort as JSON")]
    pub(crate) sort: Option<String>,
    #[arg(long, help = "Maximum number of documents to return")]
    pub(crate) limit: Option<u64>,
    #[arg(long, help = "Render output as a table")]
    #[arg(conflicts_with = "csv")]
    pub(crate) table: bool,
    #[arg(long, help = "Render output as CSV")]
    #[arg(conflicts_with = "table")]
    pub(crate) csv: bool,
    #[arg(short = 'o', long, help = "Write rendered output to a file")]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args)]
#[command(
    about = "Run a saved aggregation or an inline pipeline",
    long_about = "Run a saved aggregation by ID, or run an inline aggregation with --db/--collection/--pipeline. Shared saved aggregations can use the selected connection default_database when --db is omitted. Pipelines using $out or $merge still require --dangerously-enable-write --allow-pipeline-writes."
)]
pub(crate) struct AggArgs {
    #[arg(help = "Saved aggregation ID; omit to run an inline aggregation")]
    pub(crate) name: Option<String>,

    #[arg(long, help = "Connection name from config")]
    pub(crate) connection: Option<String>,
    #[arg(
        long,
        help = "Database name; falls back to the connection default_database"
    )]
    pub(crate) db: Option<String>,
    #[arg(long, help = "Collection name")]
    pub(crate) collection: Option<String>,
    #[arg(long, help = "Inline Mongo aggregation pipeline as JSON array")]
    pub(crate) pipeline: Option<String>,
    #[arg(long, help = "Render output as a table")]
    #[arg(conflicts_with = "csv")]
    pub(crate) table: bool,
    #[arg(long, help = "Render output as CSV")]
    #[arg(conflicts_with = "table")]
    pub(crate) csv: bool,
    #[arg(short = 'o', long, help = "Write rendered output to a file")]
    pub(crate) output: Option<PathBuf>,
}

#[derive(Args)]
#[command(
    about = "Insert one document into a collection",
    long_about = "Insert one JSON document into a collection. Requires --dangerously-enable-write. Provide the document inline with --document or load it from a file with --file."
)]
pub(crate) struct InsertArgs {
    #[arg(long, help = "Connection name from config")]
    pub(crate) connection: Option<String>,
    #[arg(
        long,
        help = "Database name; falls back to the connection default_database"
    )]
    pub(crate) db: Option<String>,
    #[arg(long, help = "Collection name")]
    pub(crate) collection: Option<String>,
    #[arg(long, help = "Document JSON passed inline", conflicts_with = "file")]
    pub(crate) document: Option<String>,
    #[arg(
        long,
        help = "Path to a file containing document JSON",
        conflicts_with = "document"
    )]
    pub(crate) file: Option<String>,
}

#[derive(Args)]
#[command(
    about = "Replace one document by _id",
    long_about = "Replace one document by _id. Requires --dangerously-enable-write. Provide the replacement document inline with --document or load it from a file with --file. If the replacement _id differs, LazyCompass keeps the --id value."
)]
pub(crate) struct UpdateArgs {
    #[arg(long, help = "Connection name from config")]
    pub(crate) connection: Option<String>,
    #[arg(
        long,
        help = "Database name; falls back to the connection default_database"
    )]
    pub(crate) db: Option<String>,
    #[arg(long, help = "Collection name")]
    pub(crate) collection: Option<String>,
    #[arg(
        long,
        help = "Document _id as JSON, for example '\"id\"' or '{\"$oid\":\"...\"}'"
    )]
    pub(crate) id: String,
    #[arg(
        long,
        help = "Replacement document JSON passed inline",
        conflicts_with = "file"
    )]
    pub(crate) document: Option<String>,
    #[arg(
        long,
        help = "Path to a file containing replacement document JSON",
        conflicts_with = "document"
    )]
    pub(crate) file: Option<String>,
}

#[derive(Args)]
#[command(
    about = "Open or update LazyCompass config",
    long_about = "Open the resolved config file in your editor or add a connection entry. Repo config is preferred when a repo is detected unless you pass --global."
)]
pub(crate) struct ConfigArgs {
    #[command(subcommand)]
    pub(crate) command: ConfigCommands,

    #[arg(
        long,
        global = true,
        group = "scope",
        help = "Use global config under ~/.config/lazycompass"
    )]
    pub(crate) global: bool,

    #[arg(
        long,
        global = true,
        group = "scope",
        help = "Use repo config under .lazycompass"
    )]
    pub(crate) repo: bool,
}

#[derive(Args)]
#[command(
    about = "Bootstrap config and add a first connection",
    long_about = "Create or reuse the target config scope, then open an interactive connection template. Repo config is preferred when a repo is detected unless you pass --global."
)]
pub(crate) struct InitArgs {
    #[arg(
        long,
        group = "scope",
        help = "Use global config under ~/.config/lazycompass"
    )]
    pub(crate) global: bool,

    #[arg(long, group = "scope", help = "Use repo config under .lazycompass")]
    pub(crate) repo: bool,
}

#[derive(Subcommand)]
pub(crate) enum ConfigCommands {
    #[command(about = "Open the resolved config file in your editor")]
    Edit,
    #[command(about = "Add a connection entry with an editable template")]
    AddConnection,
}

#[derive(Args)]
#[command(
    about = "Upgrade LazyCompass from release assets or source",
    long_about = "Download the matching GitHub release archive, verify its checksum, and replace the current binary in place. Use --version to pin a release, --repo to switch repositories, or --from-source to run cargo install from a Git checkout instead."
)]
pub(crate) struct UpgradeArgs {
    #[arg(long, help = "Install a specific release version")]
    pub(crate) version: Option<String>,
    #[arg(long, help = "Use a different GitHub repo in owner/name form")]
    pub(crate) repo: Option<String>,
    #[arg(
        long,
        help = "Use cargo install from a Git checkout instead of release assets"
    )]
    pub(crate) from_source: bool,
    #[arg(
        long,
        help = "Accepted for compatibility; upgrade no longer edits PATH profiles"
    )]
    pub(crate) no_modify_path: bool,
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn root_help_mentions_tui_and_examples() {
        let help = Cli::command().render_long_help().to_string();
        assert!(help.contains("Run LazyCompass without a subcommand to open the TUI."));
        assert!(help.contains("Enable document writes for this run"));
        assert!(help.contains("lazycompass query app.users.active_users"));
    }

    #[test]
    fn query_help_describes_saved_and_inline_modes() {
        let mut command = Cli::command();
        let query = command
            .find_subcommand_mut("query")
            .expect("query subcommand");
        let help = query.render_long_help().to_string();
        assert!(help.contains("Run a saved query by ID, or run an inline find query"));
        assert!(help.contains("Saved query ID; omit to run an inline query"));
        assert!(help.contains("Inline Mongo find filter as JSON"));
    }
}
