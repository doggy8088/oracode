use std::path::PathBuf;

use clap::{ArgGroup, Parser};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "oracode",
    version,
    about = "Export clean, Git-friendly Oracle DDL.",
    group(
        ArgGroup::new("database")
            .required(true)
            .args(["sid", "service_name"])
    )
)]
pub struct Cli {
    #[arg(long, env = "ORACODE_HOST")]
    pub host: String,

    #[arg(long, env = "ORACODE_PORT", default_value_t = 1521)]
    pub port: u16,

    #[arg(long, env = "ORACODE_USER")]
    pub user: String,

    #[arg(long, env = "ORACODE_PASSWORD")]
    pub password: String,

    #[arg(long, env = "ORACODE_SID", conflicts_with = "service_name")]
    pub sid: Option<String>,

    #[arg(
        long = "service-name",
        env = "ORACODE_SERVICE_NAME",
        conflicts_with = "sid"
    )]
    pub service_name: Option<String>,

    #[arg(long, env = "ORACODE_SCHEMA")]
    pub schema: String,

    #[arg(long, env = "ORACODE_OUTPUT", default_value = "./oracode-out")]
    pub output: PathBuf,

    #[arg(long, env = "ORACODE_KEEP_QUOTES", default_value_t = false)]
    pub keep_quotes: bool,

    #[arg(long, env = "ORACODE_CONCURRENCY", default_value_t = 8)]
    pub concurrency: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseName {
    Sid(String),
    ServiceName(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: DatabaseName,
}

impl Cli {
    pub fn connection_config(&self) -> ConnectionConfig {
        let database = match (&self.sid, &self.service_name) {
            (Some(sid), None) => DatabaseName::Sid(sid.clone()),
            (None, Some(service_name)) => DatabaseName::ServiceName(service_name.clone()),
            _ => unreachable!("clap enforces exactly one database identifier"),
        };

        ConnectionConfig {
            host: self.host.clone(),
            port: self.port,
            user: self.user.clone(),
            password: self.password.clone(),
            database,
        }
    }
}

impl ConnectionConfig {
    pub fn connect_descriptor(&self) -> String {
        let connect_data = match &self.database {
            DatabaseName::Sid(sid) => format!("SID={sid}"),
            DatabaseName::ServiceName(service_name) => format!("SERVICE_NAME={service_name}"),
        };

        format!(
            "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST={})(PORT={}))(CONNECT_DATA=({})))",
            self.host, self.port, connect_data
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        path::PathBuf,
        sync::{Mutex, MutexGuard},
    };

    use clap::{Parser, error::ErrorKind};

    use super::{Cli, ConnectionConfig, DatabaseName};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        saved: Vec<(&'static str, Option<OsString>)>,
        _lock: MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn clear_oracode_env() -> Self {
            const VARS: &[&str] = &[
                "ORACODE_HOST",
                "ORACODE_PORT",
                "ORACODE_USER",
                "ORACODE_PASSWORD",
                "ORACODE_SID",
                "ORACODE_SERVICE_NAME",
                "ORACODE_SCHEMA",
                "ORACODE_OUTPUT",
                "ORACODE_KEEP_QUOTES",
                "ORACODE_CONCURRENCY",
            ];

            let lock = ENV_LOCK.lock().expect("environment lock is poisoned");
            let saved = VARS
                .iter()
                .map(|&name| (name, std::env::var_os(name)))
                .collect::<Vec<_>>();

            for &name in VARS {
                // SAFETY: These tests serialize all ORACODE_* environment mutation with
                // ENV_LOCK and restore the previous values before releasing it.
                unsafe { std::env::remove_var(name) };
            }

            Self { saved, _lock: lock }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.saved {
                // SAFETY: The guard still holds ENV_LOCK while restoring values, so
                // ORACODE_* environment mutation remains serialized for this module.
                unsafe {
                    match value {
                        Some(value) => std::env::set_var(name, value),
                        None => std::env::remove_var(name),
                    }
                }
            }
        }
    }

    #[test]
    fn builds_service_name_descriptor() {
        let config = ConnectionConfig {
            host: "db.example.test".to_string(),
            port: 1521,
            user: "hr".to_string(),
            password: "secret".to_string(),
            database: DatabaseName::ServiceName("orclpdb1".to_string()),
        };

        assert_eq!(
            config.connect_descriptor(),
            "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=db.example.test)(PORT=1521))(CONNECT_DATA=(SERVICE_NAME=orclpdb1)))"
        );
    }

    #[test]
    fn builds_sid_descriptor() {
        let config = ConnectionConfig {
            host: "localhost".to_string(),
            port: 1522,
            user: "system".to_string(),
            password: "secret".to_string(),
            database: DatabaseName::Sid("XE".to_string()),
        };

        assert_eq!(
            config.connect_descriptor(),
            "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=localhost)(PORT=1522))(CONNECT_DATA=(SID=XE)))"
        );
    }

    #[test]
    fn parses_sid_args_with_defaults() {
        let _env = EnvGuard::clear_oracode_env();

        let cli = Cli::try_parse_from([
            "oracode",
            "--host",
            "db.example.test",
            "--user",
            "hr",
            "--password",
            "secret",
            "--sid",
            "XE",
            "--schema",
            "HR",
        ])
        .expect("valid SID arguments should parse");

        assert_eq!(cli.host, "db.example.test");
        assert_eq!(cli.port, 1521);
        assert_eq!(cli.user, "hr");
        assert_eq!(cli.password, "secret");
        assert_eq!(cli.sid.as_deref(), Some("XE"));
        assert_eq!(cli.service_name, None);
        assert_eq!(cli.schema, "HR");
        assert_eq!(cli.output, PathBuf::from("./oracode-out"));
        assert!(!cli.keep_quotes);
        assert_eq!(cli.concurrency, 8);
        assert_eq!(
            cli.connection_config(),
            ConnectionConfig {
                host: "db.example.test".to_string(),
                port: 1521,
                user: "hr".to_string(),
                password: "secret".to_string(),
                database: DatabaseName::Sid("XE".to_string()),
            }
        );
    }

    #[test]
    fn parses_service_name_args_with_overrides() {
        let _env = EnvGuard::clear_oracode_env();

        let cli = Cli::try_parse_from([
            "oracode",
            "--host",
            "db.example.test",
            "--port",
            "1522",
            "--user",
            "system",
            "--password",
            "secret",
            "--service-name",
            "orclpdb1",
            "--schema",
            "APP",
            "--output",
            "ddl",
            "--keep-quotes",
            "--concurrency",
            "3",
        ])
        .expect("valid service-name arguments should parse");

        assert_eq!(cli.host, "db.example.test");
        assert_eq!(cli.port, 1522);
        assert_eq!(cli.user, "system");
        assert_eq!(cli.password, "secret");
        assert_eq!(cli.sid, None);
        assert_eq!(cli.service_name.as_deref(), Some("orclpdb1"));
        assert_eq!(cli.schema, "APP");
        assert_eq!(cli.output, PathBuf::from("ddl"));
        assert!(cli.keep_quotes);
        assert_eq!(cli.concurrency, 3);
        assert_eq!(
            cli.connection_config(),
            ConnectionConfig {
                host: "db.example.test".to_string(),
                port: 1522,
                user: "system".to_string(),
                password: "secret".to_string(),
                database: DatabaseName::ServiceName("orclpdb1".to_string()),
            }
        );
    }

    #[test]
    fn rejects_missing_database_identifier() {
        let _env = EnvGuard::clear_oracode_env();

        let error = Cli::try_parse_from([
            "oracode",
            "--host",
            "db.example.test",
            "--user",
            "hr",
            "--password",
            "secret",
            "--schema",
            "HR",
        ])
        .expect_err("database identifier is required");

        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn rejects_conflicting_database_identifiers() {
        let _env = EnvGuard::clear_oracode_env();

        let error = Cli::try_parse_from([
            "oracode",
            "--host",
            "db.example.test",
            "--user",
            "hr",
            "--password",
            "secret",
            "--sid",
            "XE",
            "--service-name",
            "orclpdb1",
            "--schema",
            "HR",
        ])
        .expect_err("SID and service name are mutually exclusive");

        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn rejects_invalid_numeric_arguments() {
        let _env = EnvGuard::clear_oracode_env();

        let invalid_port = Cli::try_parse_from([
            "oracode",
            "--host",
            "db.example.test",
            "--port",
            "not-a-port",
            "--user",
            "hr",
            "--password",
            "secret",
            "--sid",
            "XE",
            "--schema",
            "HR",
        ]);
        assert!(invalid_port.is_err());

        let invalid_concurrency = Cli::try_parse_from([
            "oracode",
            "--host",
            "db.example.test",
            "--user",
            "hr",
            "--password",
            "secret",
            "--sid",
            "XE",
            "--schema",
            "HR",
            "--concurrency",
            "not-a-number",
        ]);
        assert!(invalid_concurrency.is_err());
    }
}
