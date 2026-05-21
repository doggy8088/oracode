use std::path::{Path, PathBuf};
use std::sync::Arc;

use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::cli::{Cli, ConnectionConfig};
use crate::db::{DbObject, OracleMetadataClient};
use crate::sanitize::{SanitizeOptions, sanitize_ddl};
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportSummary {
    pub total: usize,
    pub written: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOutcome {
    Written,
    Skipped,
}

pub async fn run(cli: Cli) -> Result<ExportSummary> {
    if cli.concurrency == 0 {
        return Err(Error::InvalidConcurrency);
    }

    let config = cli.connection_config();
    let schema = cli.schema.to_ascii_uppercase();
    let output = cli.output;
    let keep_quotes = cli.keep_quotes;
    let concurrency = cli.concurrency;

    let objects = list_objects(config.clone(), schema.clone()).await?;
    let total = objects.len();
    let progress = progress_bar(total);
    let summary = export_objects(
        config,
        schema,
        output,
        keep_quotes,
        concurrency,
        objects,
        progress.clone(),
    )
    .await;
    progress.finish_and_clear();

    match summary {
        Ok(summary) => {
            println!(
                "Exported {} objects ({} written, {} unchanged).",
                summary.total, summary.written, summary.skipped
            );
            Ok(summary)
        }
        Err(error) => Err(error),
    }
}

async fn list_objects(config: ConnectionConfig, schema: String) -> Result<Vec<DbObject>> {
    tokio::task::spawn_blocking(move || {
        let client = OracleMetadataClient::connect(&config)?;
        client.list_objects(schema.as_str())
    })
    .await?
}

async fn export_objects(
    config: ConnectionConfig,
    schema: String,
    output: PathBuf,
    keep_quotes: bool,
    concurrency: usize,
    objects: Vec<DbObject>,
    progress: ProgressBar,
) -> Result<ExportSummary> {
    let total = objects.len();
    let config = Arc::new(config);
    let schema = Arc::new(schema);
    let output = Arc::new(output);
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut tasks = JoinSet::new();

    for object in objects {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let config = Arc::clone(&config);
        let schema = Arc::clone(&schema);
        let output = Arc::clone(&output);
        let progress = progress.clone();

        tasks.spawn(async move {
            let _permit = permit;
            let object_type = object.kind.metadata_type();
            let object_name = object.name.clone();

            let result = export_one(config, schema, output, keep_quotes, object)
                .await
                .map_err(|source| Error::ExportObject {
                    object_type,
                    object_name,
                    source: Box::new(source),
                });
            progress.inc(1);
            result
        });
    }

    let mut written = 0;
    let mut skipped = 0;
    while let Some(result) = tasks.join_next().await {
        match result?? {
            WriteOutcome::Written => written += 1,
            WriteOutcome::Skipped => skipped += 1,
        }
    }

    Ok(ExportSummary {
        total,
        written,
        skipped,
    })
}

async fn export_one(
    config: Arc<ConnectionConfig>,
    schema: Arc<String>,
    output: Arc<PathBuf>,
    keep_quotes: bool,
    object: DbObject,
) -> Result<WriteOutcome> {
    let ddl_object = object.clone();
    let config = (*config).clone();
    let schema_for_fetch = (*schema).clone();

    let ddl = tokio::task::spawn_blocking(move || {
        let client = OracleMetadataClient::connect(&config)?;
        client.get_ddl(schema_for_fetch.as_str(), &ddl_object)
    })
    .await??;

    let sanitized = sanitize_ddl(&ddl, SanitizeOptions { keep_quotes });
    write_ddl_file(output.as_ref(), &object, sanitized.as_str()).await
}

pub async fn write_ddl_file(
    output_root: impl AsRef<Path>,
    object: &DbObject,
    contents: &str,
) -> Result<WriteOutcome> {
    let directory = output_root.as_ref().join(object.kind.output_dir());
    tokio::fs::create_dir_all(&directory).await?;

    let path = directory.join(format!("{}.sql", file_stem(object.name.as_str())));
    match tokio::fs::read(&path).await {
        Ok(existing) if existing == contents.as_bytes() => return Ok(WriteOutcome::Skipped),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    tokio::fs::write(path, contents).await?;
    Ok(WriteOutcome::Written)
}

fn progress_bar(total: usize) -> ProgressBar {
    let progress = ProgressBar::new(total as u64);
    let style = ProgressStyle::with_template(
        "[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} objects exported",
    )
    .unwrap()
    .progress_chars("##-");
    progress.set_style(style);
    progress
}

fn file_stem(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '#' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();

    if stem.is_empty() {
        "_".to_string()
    } else {
        stem
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::db::{DbObject, ObjectKind};

    use super::{WriteOutcome, file_stem, write_ddl_file};

    #[test]
    fn sanitizes_file_stems() {
        assert_eq!(file_stem("EMPLOYEES"), "EMPLOYEES");
        assert_eq!(file_stem("Weird Name/With:Chars"), "Weird_Name_With_Chars");
        assert_eq!(file_stem(""), "_");
    }

    #[tokio::test]
    async fn writes_then_skips_unchanged_content() {
        let root = std::env::temp_dir().join(format!(
            "oracode-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let object = DbObject {
            name: "EMPLOYEES".to_string(),
            kind: ObjectKind::Table,
        };

        let first = write_ddl_file(&root, &object, "CREATE TABLE EMPLOYEES;\n")
            .await
            .unwrap();
        let second = write_ddl_file(&root, &object, "CREATE TABLE EMPLOYEES;\n")
            .await
            .unwrap();

        assert_eq!(first, WriteOutcome::Written);
        assert_eq!(second, WriteOutcome::Skipped);

        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
