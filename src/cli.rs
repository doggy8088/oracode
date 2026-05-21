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
    use super::{ConnectionConfig, DatabaseName};

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
}
