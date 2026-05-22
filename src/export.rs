use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::cli::{Cli, ConnectionConfig};
use crate::db::{DbObject, MetadataOptions, ObjectSelection, OracleMetadataClient};
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

struct ExportRequest {
    config: ConnectionConfig,
    metadata_options: MetadataOptions,
    schema: String,
    output: PathBuf,
    sanitize_options: SanitizeOptions,
    concurrency: usize,
}

pub async fn run(cli: Cli) -> Result<ExportSummary> {
    if cli.concurrency == 0 {
        return Err(Error::InvalidConcurrency);
    }

    let config = cli.connection_config();
    let schema = cli.schema;
    let output = cli.output;
    let metadata_options = MetadataOptions {
        lossless: cli.lossless,
    };
    let sanitize_options = SanitizeOptions {
        keep_quotes: cli.keep_quotes || cli.lossless,
        preserve_editioning: cli.lossless,
    };
    let object_selection = ObjectSelection::from_selectors(&cli.include, &cli.exclude)?;
    let concurrency = cli.concurrency;

    let (schema, objects) =
        list_objects(config.clone(), metadata_options, schema, object_selection).await?;
    let total = objects.len();
    let progress = progress_bar(total);
    let request = ExportRequest {
        config,
        metadata_options,
        schema,
        output,
        sanitize_options,
        concurrency,
    };
    let summary = export_objects(request, objects, progress.clone()).await;
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

async fn list_objects(
    config: ConnectionConfig,
    metadata_options: MetadataOptions,
    schema: String,
    object_selection: ObjectSelection,
) -> Result<(String, Vec<DbObject>)> {
    tokio::task::spawn_blocking(move || {
        let client = OracleMetadataClient::connect(&config, metadata_options)?;
        let schema = client.resolve_schema(schema.as_str())?;
        let objects = client.list_objects(schema.as_str(), &object_selection)?;
        Ok((schema, objects))
    })
    .await?
}

async fn export_objects(
    request: ExportRequest,
    objects: Vec<DbObject>,
    progress: ProgressBar,
) -> Result<ExportSummary> {
    let total = objects.len();
    reject_filename_collisions(request.output.as_path(), &objects)?;

    let config = Arc::new(request.config);
    let schema = Arc::new(request.schema);
    let output = Arc::new(request.output);
    let metadata_options = request.metadata_options;
    let sanitize_options = request.sanitize_options;
    let semaphore = Arc::new(Semaphore::new(request.concurrency));
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

            let result = export_one(
                config,
                metadata_options,
                schema,
                output,
                sanitize_options,
                object,
            )
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
    metadata_options: MetadataOptions,
    schema: Arc<String>,
    output: Arc<PathBuf>,
    sanitize_options: SanitizeOptions,
    object: DbObject,
) -> Result<WriteOutcome> {
    let ddl_object = object.clone();
    let config = (*config).clone();
    let schema_for_fetch = (*schema).clone();

    let ddl = tokio::task::spawn_blocking(move || {
        let client = OracleMetadataClient::connect(&config, metadata_options)?;
        client.get_ddl(schema_for_fetch.as_str(), &ddl_object)
    })
    .await??;

    let sanitized = sanitize_ddl(&ddl, sanitize_options);
    write_ddl_file(output.as_ref(), &object, sanitized.as_str()).await
}

pub async fn write_ddl_file(
    output_root: impl AsRef<Path>,
    object: &DbObject,
    contents: &str,
) -> Result<WriteOutcome> {
    let directory = output_root.as_ref().join(object.kind.output_dir());
    tokio::fs::create_dir_all(&directory).await?;

    let path = output_path(output_root.as_ref(), object);
    match tokio::fs::read(&path).await {
        Ok(existing) if existing == contents.as_bytes() => return Ok(WriteOutcome::Skipped),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    tokio::fs::write(path, contents).await?;
    Ok(WriteOutcome::Written)
}

fn reject_filename_collisions(output_root: &Path, objects: &[DbObject]) -> Result<()> {
    let mut paths: HashMap<PathBuf, &DbObject> = HashMap::new();

    for object in objects {
        let path = output_path(output_root, object);
        if let Some(first) = paths.insert(path.clone(), object) {
            return Err(Error::FilenameCollision {
                path: path.display().to_string(),
                first_object_type: first.kind.metadata_type(),
                first_object_name: first.name.clone(),
                second_object_type: object.kind.metadata_type(),
                second_object_name: object.name.clone(),
            });
        }
    }

    Ok(())
}

fn output_path(output_root: &Path, object: &DbObject) -> PathBuf {
    output_root
        .join(object.kind.output_dir())
        .join(format!("{}.sql", file_stem(object.name.as_str())))
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
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::Error;
    use crate::db::{DbObject, ObjectKind};

    use super::{WriteOutcome, file_stem, reject_filename_collisions, write_ddl_file};

    fn unique_test_root(test_name: &str) -> std::path::PathBuf {
        std::env::current_dir()
            .unwrap()
            .join("target")
            .join("oracode-export-tests")
            .join(format!(
                "{}-{}",
                test_name,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ))
    }

    #[test]
    fn sanitizes_file_stems() {
        assert_eq!(file_stem("EMPLOYEES"), "EMPLOYEES");
        assert_eq!(file_stem("Weird Name/With:Chars"), "Weird_Name_With_Chars");
        assert_eq!(file_stem(""), "_");
    }

    #[test]
    fn rejects_filename_collisions_before_export() {
        let objects = vec![
            DbObject {
                name: "A/B".to_string(),
                kind: ObjectKind::Table,
            },
            DbObject {
                name: "A:B".to_string(),
                kind: ObjectKind::Table,
            },
        ];

        assert!(reject_filename_collisions(Path::new("out"), &objects).is_err());
    }

    #[tokio::test]
    async fn writes_then_skips_unchanged_content() {
        let root = unique_test_root("writes-then-skips-unchanged-content");
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

    #[tokio::test]
    async fn writes_object_grants_under_object_grant_directory() {
        let root = unique_test_root("writes-object-grants");
        let object = DbObject {
            name: "EMPLOYEES".to_string(),
            kind: ObjectKind::ObjectGrant,
        };

        let outcome = write_ddl_file(&root, &object, "GRANT SELECT ON EMPLOYEES TO REPORTING;\n")
            .await
            .unwrap();
        let written = tokio::fs::read_to_string(root.join("OBJECT_GRANT").join("EMPLOYEES.sql"))
            .await
            .unwrap();

        assert_eq!(outcome, WriteOutcome::Written);
        assert_eq!(written, "GRANT SELECT ON EMPLOYEES TO REPORTING;\n");

        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn writes_dependent_metadata_under_its_own_directory() {
        let root = unique_test_root("writes-dependent-metadata");
        let constraint = DbObject {
            name: "EMPLOYEES".to_string(),
            kind: ObjectKind::ObjectConstraint,
        };
        let comment = DbObject {
            name: "EMPLOYEES".to_string(),
            kind: ObjectKind::ObjectComment,
        };

        write_ddl_file(
            &root,
            &constraint,
            "ALTER TABLE EMPLOYEES ADD CONSTRAINT EMP_PK PRIMARY KEY (ID);\n",
        )
        .await
        .unwrap();
        write_ddl_file(
            &root,
            &comment,
            "COMMENT ON TABLE EMPLOYEES IS 'Employees';\n",
        )
        .await
        .unwrap();

        let constraint_sql =
            tokio::fs::read_to_string(root.join("CONSTRAINT").join("EMPLOYEES.sql"))
                .await
                .unwrap();
        let comment_sql = tokio::fs::read_to_string(root.join("COMMENT").join("EMPLOYEES.sql"))
            .await
            .unwrap();

        assert_eq!(
            constraint_sql,
            "ALTER TABLE EMPLOYEES ADD CONSTRAINT EMP_PK PRIMARY KEY (ID);\n"
        );
        assert_eq!(comment_sql, "COMMENT ON TABLE EMPLOYEES IS 'Employees';\n");

        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn overwrites_changed_content_and_reports_written() {
        let root = unique_test_root("overwrites-changed-content");
        let object = DbObject {
            name: "EMPLOYEES".to_string(),
            kind: ObjectKind::Table,
        };

        let first = write_ddl_file(&root, &object, "CREATE TABLE EMPLOYEES (ID NUMBER);\n")
            .await
            .unwrap();
        let second = write_ddl_file(
            &root,
            &object,
            "CREATE TABLE EMPLOYEES (ID NUMBER, NAME VARCHAR2(30));\n",
        )
        .await
        .unwrap();
        let written = tokio::fs::read_to_string(root.join("TABLE").join("EMPLOYEES.sql"))
            .await
            .unwrap();

        assert_eq!(first, WriteOutcome::Written);
        assert_eq!(second, WriteOutcome::Written);
        assert_eq!(
            written,
            "CREATE TABLE EMPLOYEES (ID NUMBER, NAME VARCHAR2(30));\n"
        );

        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn writes_sanitized_file_name_under_object_type_directory() {
        let root = unique_test_root("writes-sanitized-file-name");
        let object = DbObject {
            name: "EMP/DETAIL:VIEW".to_string(),
            kind: ObjectKind::View,
        };

        let outcome = write_ddl_file(
            &root,
            &object,
            "CREATE VIEW EMP_DETAIL AS SELECT 1 FROM DUAL;\n",
        )
        .await
        .unwrap();
        let written = tokio::fs::read_to_string(root.join("VIEW").join("EMP_DETAIL_VIEW.sql"))
            .await
            .unwrap();

        assert_eq!(outcome, WriteOutcome::Written);
        assert_eq!(written, "CREATE VIEW EMP_DETAIL AS SELECT 1 FROM DUAL;\n");

        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[test]
    fn filename_collision_reports_both_conflicting_objects() {
        let objects = vec![
            DbObject {
                name: "A/B".to_string(),
                kind: ObjectKind::PackageSpec,
            },
            DbObject {
                name: "A:B".to_string(),
                kind: ObjectKind::PackageSpec,
            },
        ];

        let error = reject_filename_collisions(Path::new("out"), &objects).unwrap_err();

        match error {
            Error::FilenameCollision {
                path,
                first_object_type,
                first_object_name,
                second_object_type,
                second_object_name,
            } => {
                assert!(path.ends_with("PACKAGE_SPEC/A_B.sql"));
                assert_eq!(first_object_type, "PACKAGE_SPEC");
                assert_eq!(first_object_name, "A/B");
                assert_eq!(second_object_type, "PACKAGE_SPEC");
                assert_eq!(second_object_name, "A:B");
            }
            other => panic!("expected filename collision, got {other:?}"),
        }
    }

    #[test]
    fn allows_same_file_stem_in_different_object_type_directories() {
        let objects = vec![
            DbObject {
                name: "EMPLOYEES".to_string(),
                kind: ObjectKind::Table,
            },
            DbObject {
                name: "EMPLOYEES".to_string(),
                kind: ObjectKind::View,
            },
        ];

        reject_filename_collisions(Path::new("out"), &objects).unwrap();
    }
}
