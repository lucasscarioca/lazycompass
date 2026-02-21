use anyhow::{Result, bail};
use lazycompass_core::Config;
use lazycompass_mongo::MongoExecutor;

pub(crate) fn resolve_database_arg(
    config: &Config,
    connection: Option<&str>,
    database: Option<String>,
    missing_error: impl Into<String>,
) -> Result<String> {
    if let Some(database) = database {
        let trimmed = database.trim();
        if !trimmed.is_empty() {
            return Ok(database);
        }
    }

    let missing_error = missing_error.into();
    let executor = MongoExecutor::new();
    match executor.resolve_connection(config, connection) {
        Ok(connection) => {
            if let Some(default_database) = connection
                .default_database
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Ok(default_database.to_string());
            }
            bail!("{missing_error}; set --db or connections[].default_database");
        }
        Err(error) => {
            if connection.is_some() {
                return Err(error);
            }
            bail!(
                "{missing_error}; set --db or pass --connection with connections[].default_database"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use lazycompass_core::{Config, ConnectionSpec};

    use super::resolve_database_arg;

    fn config_with_connections(connections: Vec<ConnectionSpec>) -> Config {
        Config {
            connections,
            ..Config::default()
        }
    }

    fn connection(name: &str, default_database: Option<&str>) -> ConnectionSpec {
        ConnectionSpec {
            name: name.to_string(),
            uri: format!("mongodb://{name}:27017"),
            default_database: default_database.map(ToString::to_string),
        }
    }

    #[test]
    fn resolve_database_arg_prefers_explicit_db() {
        let config = config_with_connections(vec![connection("local", Some("default"))]);
        let database = resolve_database_arg(
            &config,
            Some("local"),
            Some("override".to_string()),
            "--db is required",
        )
        .expect("resolve database");
        assert_eq!(database, "override");
    }

    #[test]
    fn resolve_database_arg_uses_connection_default() {
        let config = config_with_connections(vec![connection("local", Some("default"))]);
        let database = resolve_database_arg(&config, Some("local"), None, "--db is required")
            .expect("resolve database");
        assert_eq!(database, "default");
    }

    #[test]
    fn resolve_database_arg_surfaces_unknown_explicit_connection() {
        let config = config_with_connections(vec![connection("local", Some("default"))]);
        let err = resolve_database_arg(&config, Some("missing"), None, "--db is required")
            .expect_err("expected unknown connection");
        assert!(err.to_string().contains("connection 'missing' not found"));
    }

    #[test]
    fn resolve_database_arg_errors_with_hint_when_connection_is_ambiguous() {
        let config = config_with_connections(vec![
            connection("primary", Some("app")),
            connection("secondary", Some("audit")),
        ]);
        let err = resolve_database_arg(&config, None, None, "--db is required")
            .expect_err("expected missing database");
        assert!(
            err.to_string()
                .contains("set --db or pass --connection with connections[].default_database")
        );
    }
}
