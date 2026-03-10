use anyhow::{Context, Result};
use lazycompass_core::ConnectionSpec;
use lazycompass_storage::{
    ConfigPaths, append_connection_to_global_config, append_connection_to_repo_config,
    ensure_not_symlinked_file, ensure_secure_dir, write_secure_file,
};
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};

use crate::cli::{ConfigArgs, ConfigCommands};
use crate::editor::{create_secure_temp_file, open_in_editor};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ConfigScope {
    Global,
    Repo,
}

pub(crate) fn run_config(args: ConfigArgs) -> Result<()> {
    let cwd = std::env::current_dir().context("unable to resolve current directory")?;
    let paths = ConfigPaths::resolve_from(&cwd)?;
    let scope = resolve_config_scope(&paths, args.global, args.repo);

    match args.command {
        ConfigCommands::Edit => run_config_edit(&paths, scope),
        ConfigCommands::AddConnection(command_args) => {
            run_config_add_connection(&paths, scope, command_args.editor)
        }
    }
}

pub(crate) fn resolve_config_scope(paths: &ConfigPaths, global: bool, repo: bool) -> ConfigScope {
    if global {
        return ConfigScope::Global;
    }
    if repo || paths.repo_config_path().is_some() {
        return ConfigScope::Repo;
    }
    ConfigScope::Global
}

fn run_config_edit(paths: &ConfigPaths, scope: ConfigScope) -> Result<()> {
    let config_path = match scope {
        ConfigScope::Global => paths.global_config_path(),
        ConfigScope::Repo => paths.repo_config_path().ok_or_else(|| {
            anyhow::anyhow!(
                "no repository config found; run inside a repo with .lazycompass or use --global"
            )
        })?,
    };

    if let Some(parent) = config_path.parent() {
        ensure_secure_dir(parent)?;
    }
    ensure_not_symlinked_file(&config_path)?;

    if !config_path.exists() {
        let default_config = r#"# LazyCompass Configuration
# Global config: ~/.config/lazycompass/config.toml
# Repo config: .lazycompass/config.toml (overrides global)

# Example connection (remove if not needed):
# [[connections]]
# name = "local"
# uri = "mongodb://localhost:27017"
# default_database = "mydb"

[theme]
# name = "classic"

[logging]
# level = "warn"
# file = "lazycompass.log"
# max_size_mb = 10
# max_backups = 3

[timeouts]
# connect_ms = 10000
# query_ms = 30000

# allow_insecure = false

# Writes are enabled per run only:
# lazycompass --dangerously-enable-write …
# lazycompass --dangerously-enable-write --allow-pipeline-writes …
"#;
        write_secure_file(&config_path, default_config, false)
            .with_context(|| format!("unable to create config file {}", config_path.display()))?;
    }

    open_in_editor(&config_path)?;

    println!("config opened in editor: {}", config_path.display());
    Ok(())
}

pub(crate) fn run_config_add_connection(
    paths: &ConfigPaths,
    scope: ConfigScope,
    editor: bool,
) -> Result<()> {
    if let ConfigScope::Repo = scope {
        paths.repo_config_path().ok_or_else(|| {
            anyhow::anyhow!(
                "no repository config found; run inside a repo with .lazycompass or use --global"
            )
        })?;
    }

    let connection = if editor {
        collect_connection_via_editor()?
    } else {
        ensure_interactive_prompt_support()?;
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut input = stdin.lock();
        let mut output = stdout.lock();
        collect_connection_interactively(&mut input, &mut output)?
    };

    let runtime = tokio::runtime::Runtime::new().context("unable to start async runtime")?;

    let config_path = match scope {
        ConfigScope::Global => {
            runtime.block_on(append_connection_to_global_config(paths, &connection))?
        }
        ConfigScope::Repo => {
            runtime.block_on(append_connection_to_repo_config(paths, &connection))?
        }
    };

    println!(
        "connection '{}' added to {}",
        connection.name,
        config_path.display()
    );
    Ok(())
}

fn ensure_interactive_prompt_support() -> Result<()> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        return Ok(());
    }

    anyhow::bail!("interactive prompts require a TTY; rerun with --editor or edit config manually");
}

fn collect_connection_via_editor() -> Result<ConnectionSpec> {
    let template = r#"# Edit the connection details below and save the file.
# Lines starting with # are comments and will be ignored.
# URI values can use env vars like ${MONGO_URI}.

name = "my-connection"
uri = "${MONGO_URI}"
default_database = "mydb"
"#;

    let temp_path = create_secure_temp_file("connection", "toml", template)?;

    open_in_editor(&temp_path)?;

    let edited_content = fs::read_to_string(&temp_path)
        .with_context(|| format!("unable to read edited file {}", temp_path.display()))?;

    let _ = fs::remove_file(&temp_path);

    let toml_content: String = edited_content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    let connection: ConnectionSpec = toml::from_str(&toml_content).with_context(
        || "invalid TOML in connection definition; expected fields: name, uri, default_database",
    )?;

    validate_connection(connection)
}

fn collect_connection_interactively<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<ConnectionSpec> {
    writeln!(
        output,
        "Add a MongoDB connection. URI values can use env vars like ${{MONGO_URI}}."
    )
    .context("unable to write prompt")?;

    let uri = prompt_required(
        input,
        output,
        "MongoDB URI",
        "Connection URI is required. Example: mongodb://localhost:27017 or ${MONGO_URI}.",
        "connection uri cannot be empty",
    )?;
    let default_database = prompt_optional(
        input,
        output,
        "Default database (optional)",
        "Leave blank to skip.",
    )?;
    let name = prompt_required(
        input,
        output,
        "Connection name",
        "Pick a short stable label like local, dev, or staging.",
        "connection name cannot be empty",
    )?;

    validate_connection(ConnectionSpec {
        name,
        uri,
        default_database,
    })
}

fn prompt_required<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    label: &str,
    hint: &str,
    error_message: &str,
) -> Result<String> {
    loop {
        let value = prompt_line(input, output, label, hint)?;
        if !value.trim().is_empty() {
            return Ok(value);
        }
        writeln!(output, "{error_message}").context("unable to write prompt")?;
    }
}

fn prompt_optional<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    label: &str,
    hint: &str,
) -> Result<Option<String>> {
    let value = prompt_line(input, output, label, hint)?;
    if value.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(value))
}

fn prompt_line<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    label: &str,
    hint: &str,
) -> Result<String> {
    write!(output, "{label}: ").context("unable to write prompt")?;
    output.flush().context("unable to flush prompt")?;

    let mut line = String::new();
    let bytes = input
        .read_line(&mut line)
        .with_context(|| format!("unable to read {label}"))?;
    if bytes == 0 {
        anyhow::bail!("prompt aborted");
    }

    let value = line.trim().to_string();
    if value.is_empty() {
        writeln!(output, "{hint}").context("unable to write prompt")?;
    }
    Ok(value)
}

fn validate_connection(connection: ConnectionSpec) -> Result<ConnectionSpec> {
    if connection.name.trim().is_empty() {
        anyhow::bail!("connection name cannot be empty");
    }
    if connection.uri.trim().is_empty() {
        anyhow::bail!("connection uri cannot be empty");
    }
    Ok(connection)
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use lazycompass_storage::ConfigPaths;

    #[cfg(unix)]
    use super::run_config_edit;
    use super::{ConfigScope, collect_connection_interactively, resolve_config_scope};
    use crate::cli::ConfigCommands;
    use clap::{CommandFactory, Parser};

    #[test]
    fn resolve_config_scope_defaults_to_repo_when_available() {
        let paths = ConfigPaths {
            global_root: "/tmp/global".into(),
            repo_root: Some("/tmp/repo".into()),
        };
        assert!(matches!(
            resolve_config_scope(&paths, false, false),
            ConfigScope::Repo
        ));
    }

    #[test]
    fn resolve_config_scope_defaults_to_global_without_repo() {
        let paths = ConfigPaths {
            global_root: "/tmp/global".into(),
            repo_root: None,
        };
        assert!(matches!(
            resolve_config_scope(&paths, false, false),
            ConfigScope::Global
        ));
    }

    #[cfg(unix)]
    #[test]
    fn run_config_edit_rejects_symlinked_config_paths() {
        let root = std::env::temp_dir().join(format!(
            "lazycompass_config_edit_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create root");
        let target = root.join("target.toml");
        let link_root = root.join("global");
        fs::create_dir_all(&link_root).expect("create global root");
        fs::write(&target, "").expect("write target");
        symlink(&target, link_root.join("config.toml")).expect("create symlink");

        let paths = ConfigPaths {
            global_root: link_root,
            repo_root: None,
        };
        let err = run_config_edit(&paths, ConfigScope::Global).expect_err("expected symlink error");
        assert!(err.to_string().contains("symlinked file"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn add_connection_help_mentions_cli_prompts_and_editor() {
        let mut command = crate::cli::Cli::command();
        let config = command
            .find_subcommand_mut("config")
            .expect("config command");
        let add_connection = config
            .find_subcommand_mut("add-connection")
            .expect("add-connection command");
        let help = add_connection.render_long_help().to_string();
        assert!(help.contains("CLI prompts by default"));
        assert!(help.contains("--editor"));
    }

    #[test]
    fn collect_connection_interactively_accepts_env_uri_and_blank_db() {
        let input = b"${MONGO_URI}\n\nlocal\n";
        let mut reader = &input[..];
        let mut output = Vec::new();

        let connection =
            collect_connection_interactively(&mut reader, &mut output).expect("collect prompts");

        assert_eq!(connection.uri, "${MONGO_URI}");
        assert_eq!(connection.default_database, None);
        assert_eq!(connection.name, "local");
    }

    #[test]
    fn collect_connection_interactively_retries_blank_required_fields() {
        let input = b"\n mongodb://localhost:27017 \n\n\n dev \n";
        let mut reader = &input[..];
        let mut output = Vec::new();

        let connection =
            collect_connection_interactively(&mut reader, &mut output).expect("collect prompts");
        let rendered = String::from_utf8(output).expect("utf8");

        assert_eq!(connection.uri, "mongodb://localhost:27017");
        assert_eq!(connection.name, "dev");
        assert!(rendered.contains("connection uri cannot be empty"));
        assert!(rendered.contains("connection name cannot be empty"));
    }

    #[test]
    fn config_add_connection_subcommand_accepts_editor_flag() {
        let parsed =
            crate::cli::Cli::parse_from(["lazycompass", "config", "add-connection", "--editor"]);
        match parsed.command.expect("subcommand") {
            crate::cli::Commands::Config(args) => match args.command {
                ConfigCommands::AddConnection(crate::cli::AddConnectionArgs { editor }) => {
                    assert!(editor)
                }
                _ => panic!("expected add-connection"),
            },
            _ => panic!("expected config command"),
        }
    }
}
