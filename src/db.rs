use std::collections::BTreeSet;

use oracle::Connection;

use crate::cli::ConnectionConfig;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectKind {
    Table,
    View,
    Procedure,
    Function,
    PackageSpec,
    PackageBody,
    Trigger,
    Sequence,
    TypeSpec,
    TypeBody,
    Index,
    Synonym,
    MaterializedView,
    DatabaseLink,
    ObjectConstraint,
    RefConstraint,
    ObjectComment,
    ObjectGrant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbObject {
    pub name: String,
    pub kind: ObjectKind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MetadataOptions {
    pub lossless: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataTransformResult {
    pub name: &'static str,
    pub applied: bool,
    pub error_code: Option<i32>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DbCapabilities {
    pub server_version_text: Option<String>,
    pub container_name: Option<String>,
    pub has_all_tab_identity_cols: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectSelection {
    kinds: BTreeSet<ObjectKind>,
}

const CORE_OBJECT_KINDS: &[ObjectKind] = &[
    ObjectKind::Table,
    ObjectKind::View,
    ObjectKind::Procedure,
    ObjectKind::Function,
    ObjectKind::PackageSpec,
    ObjectKind::PackageBody,
    ObjectKind::Trigger,
    ObjectKind::Sequence,
    ObjectKind::TypeSpec,
    ObjectKind::TypeBody,
];

const ALL_OBJECT_KINDS: &[ObjectKind] = &[
    ObjectKind::Table,
    ObjectKind::View,
    ObjectKind::Procedure,
    ObjectKind::Function,
    ObjectKind::PackageSpec,
    ObjectKind::PackageBody,
    ObjectKind::Trigger,
    ObjectKind::Sequence,
    ObjectKind::TypeSpec,
    ObjectKind::TypeBody,
    ObjectKind::Index,
    ObjectKind::Synonym,
    ObjectKind::MaterializedView,
    ObjectKind::DatabaseLink,
    ObjectKind::ObjectConstraint,
    ObjectKind::RefConstraint,
    ObjectKind::ObjectComment,
    ObjectKind::ObjectGrant,
];

const PACKAGE_KINDS: &[ObjectKind] = &[ObjectKind::PackageSpec, ObjectKind::PackageBody];
const TYPE_KINDS: &[ObjectKind] = &[ObjectKind::TypeSpec, ObjectKind::TypeBody];
const EMPTY_KINDS: &[ObjectKind] = &[];

impl ObjectKind {
    pub fn from_all_objects_type(value: &str) -> Option<Self> {
        match value {
            "TABLE" => Some(Self::Table),
            "VIEW" => Some(Self::View),
            "PROCEDURE" => Some(Self::Procedure),
            "FUNCTION" => Some(Self::Function),
            "PACKAGE" => Some(Self::PackageSpec),
            "PACKAGE BODY" => Some(Self::PackageBody),
            "TRIGGER" => Some(Self::Trigger),
            "SEQUENCE" => Some(Self::Sequence),
            "TYPE" => Some(Self::TypeSpec),
            "TYPE BODY" => Some(Self::TypeBody),
            "INDEX" => Some(Self::Index),
            "SYNONYM" => Some(Self::Synonym),
            "MATERIALIZED VIEW" => Some(Self::MaterializedView),
            "DATABASE LINK" => Some(Self::DatabaseLink),
            _ => None,
        }
    }

    pub fn all_objects_type(self) -> &'static str {
        match self {
            Self::Table => "TABLE",
            Self::View => "VIEW",
            Self::Procedure => "PROCEDURE",
            Self::Function => "FUNCTION",
            Self::PackageSpec => "PACKAGE",
            Self::PackageBody => "PACKAGE BODY",
            Self::Trigger => "TRIGGER",
            Self::Sequence => "SEQUENCE",
            Self::TypeSpec => "TYPE",
            Self::TypeBody => "TYPE BODY",
            Self::Index => "INDEX",
            Self::Synonym => "SYNONYM",
            Self::MaterializedView => "MATERIALIZED VIEW",
            Self::DatabaseLink => "DATABASE LINK",
            Self::ObjectConstraint => "CONSTRAINT",
            Self::RefConstraint => "REF_CONSTRAINT",
            Self::ObjectComment => "COMMENT",
            Self::ObjectGrant => "OBJECT_GRANT",
        }
    }

    fn list_query_object_type(self) -> Option<&'static str> {
        match self {
            Self::ObjectConstraint
            | Self::RefConstraint
            | Self::ObjectComment
            | Self::ObjectGrant => None,
            _ => Some(self.all_objects_type()),
        }
    }

    pub fn metadata_type(self) -> &'static str {
        match self {
            Self::Table => "TABLE",
            Self::View => "VIEW",
            Self::Procedure => "PROCEDURE",
            Self::Function => "FUNCTION",
            Self::PackageSpec => "PACKAGE_SPEC",
            Self::PackageBody => "PACKAGE_BODY",
            Self::Trigger => "TRIGGER",
            Self::Sequence => "SEQUENCE",
            Self::TypeSpec => "TYPE_SPEC",
            Self::TypeBody => "TYPE_BODY",
            Self::Index => "INDEX",
            Self::Synonym => "SYNONYM",
            Self::MaterializedView => "MATERIALIZED_VIEW",
            Self::DatabaseLink => "DB_LINK",
            Self::ObjectConstraint => "CONSTRAINT",
            Self::RefConstraint => "REF_CONSTRAINT",
            Self::ObjectComment => "COMMENT",
            Self::ObjectGrant => "OBJECT_GRANT",
        }
    }

    pub fn output_dir(self) -> &'static str {
        self.metadata_type()
    }
}

impl ObjectSelection {
    pub fn core() -> Self {
        Self {
            kinds: CORE_OBJECT_KINDS.iter().copied().collect(),
        }
    }

    pub fn from_selectors(include: &[String], exclude: &[String]) -> Result<Self> {
        let mut selection = Self::core();

        for selector in include {
            add_selector(&mut selection.kinds, selector)?;
        }
        for selector in exclude {
            remove_selector(&mut selection.kinds, selector)?;
        }

        Ok(selection)
    }

    pub fn contains(&self, kind: ObjectKind) -> bool {
        self.kinds.contains(&kind)
    }

    fn all_objects_types(&self) -> BTreeSet<&'static str> {
        self.kinds
            .iter()
            .filter_map(|kind| kind.list_query_object_type())
            .collect()
    }
}

impl Default for ObjectSelection {
    fn default() -> Self {
        Self::core()
    }
}

pub fn build_list_objects_sql(
    capabilities: &DbCapabilities,
    selection: &ObjectSelection,
) -> String {
    let object_types = selection
        .all_objects_types()
        .into_iter()
        .map(|object_type| format!("'{object_type}'"))
        .collect::<Vec<_>>();

    if object_types.is_empty() {
        return r#"
            SELECT o.object_name, o.object_type
            FROM all_objects o
            WHERE o.owner = :schema
              AND 1 = 0
            ORDER BY o.object_type, o.object_name
        "#
        .to_string();
    }

    let mut predicates = vec![
        "o.owner = :schema".to_string(),
        format!("o.object_type IN ({})", object_types.join(", ")),
        "o.object_name NOT LIKE 'BIN$%'".to_string(),
        identity_sequence_predicate(capabilities.has_all_tab_identity_cols),
    ];

    if selection.contains(ObjectKind::Index) {
        predicates.push(
            r#"(
                o.object_type != 'INDEX'
                OR NOT EXISTS (
                  SELECT 1
                  FROM all_constraints c
                  WHERE c.owner = o.owner
                    AND c.index_name = o.object_name
                    AND c.constraint_type IN ('P', 'U')
                )
              )"#
            .to_string(),
        );
    }

    format!(
        r#"
            SELECT o.object_name, o.object_type
            FROM all_objects o
            WHERE {}
            ORDER BY o.object_type, o.object_name
        "#,
        predicates.join("\n              AND ")
    )
}

pub fn build_list_object_grants_sql() -> &'static str {
    r#"
            SELECT DISTINCT p.table_name, o.object_type
            FROM all_tab_privs p
            JOIN all_objects o
              ON o.owner = p.table_schema
             AND o.object_name = p.table_name
            WHERE p.table_schema = :schema
              AND p.table_name NOT LIKE 'BIN$%'
              AND o.object_type IN (
                'TABLE',
                'VIEW',
                'PROCEDURE',
                'FUNCTION',
                'PACKAGE',
                'SEQUENCE',
                'TYPE',
                'MATERIALIZED VIEW'
              )
            ORDER BY o.object_type, p.table_name
        "#
}

pub fn build_list_object_constraints_sql(ref_constraints: bool) -> &'static str {
    if ref_constraints {
        r#"
            SELECT DISTINCT table_name
            FROM all_constraints
            WHERE owner = :schema
              AND constraint_type = 'R'
              AND constraint_name NOT LIKE 'BIN$%'
            ORDER BY table_name
        "#
    } else {
        r#"
            SELECT DISTINCT table_name
            FROM all_constraints
            WHERE owner = :schema
              AND constraint_type != 'R'
              AND constraint_name NOT LIKE 'BIN$%'
            ORDER BY table_name
        "#
    }
}

pub fn build_list_object_comments_sql() -> &'static str {
    r#"
            SELECT table_name
            FROM all_tab_comments
            WHERE owner = :schema
              AND comments IS NOT NULL
            UNION
            SELECT table_name
            FROM all_col_comments
            WHERE owner = :schema
              AND comments IS NOT NULL
            ORDER BY table_name
        "#
}

pub struct OracleMetadataClient {
    connection: Connection,
    transform_results: Vec<MetadataTransformResult>,
}

impl OracleMetadataClient {
    pub fn connect(config: &ConnectionConfig, metadata_options: MetadataOptions) -> Result<Self> {
        let connection = Connection::connect(
            config.user.as_str(),
            config.password.as_str(),
            config.connect_descriptor().as_str(),
        )?;
        let transform_results = configure_metadata(&connection, metadata_options)?;
        let client = Self {
            connection,
            transform_results,
        };
        Ok(client)
    }

    pub fn resolve_schema(&self, schema: &str) -> Result<String> {
        let (schema, explicit_quoted) = schema_lookup_value(schema);

        if let Some(owner) = self.find_schema(schema.as_str())? {
            return Ok(owner);
        }

        let uppercase = schema.to_ascii_uppercase();
        if !explicit_quoted
            && uppercase != schema
            && let Some(owner) = self.find_schema(uppercase.as_str())?
        {
            return Ok(owner);
        }

        Err(Error::SchemaNotFound(schema))
    }

    pub fn detect_capabilities(&self) -> Result<DbCapabilities> {
        let server_version_text = self
            .connection
            .server_version()
            .ok()
            .map(|(_, banner)| banner);
        let container_name = self
            .connection
            .query_row_as::<String>("SELECT SYS_CONTEXT('USERENV', 'CON_NAME') FROM dual", &[])
            .ok()
            .filter(|value| !value.trim().is_empty());
        let has_all_tab_identity_cols =
            self.probe_query_available("SELECT 1 FROM all_tab_identity_cols WHERE 1 = 0")?;

        Ok(DbCapabilities {
            server_version_text,
            container_name,
            has_all_tab_identity_cols,
        })
    }

    pub fn transform_results(&self) -> &[MetadataTransformResult] {
        &self.transform_results
    }

    pub fn list_objects(&self, schema: &str, selection: &ObjectSelection) -> Result<Vec<DbObject>> {
        let capabilities = self.detect_capabilities()?;
        let sql = build_list_objects_sql(&capabilities, selection);
        let rows = self.connection.query(&sql, &[&schema])?;
        let mut objects = Vec::new();
        for row_result in rows {
            let row = row_result?;
            let name: String = row.get("OBJECT_NAME")?;
            let object_type: String = row.get("OBJECT_TYPE")?;
            let kind = ObjectKind::from_all_objects_type(object_type.as_str())
                .ok_or_else(|| Error::UnsupportedObjectType(object_type.clone()))?;
            objects.push(DbObject { name, kind });
        }
        if selection.contains(ObjectKind::ObjectConstraint) {
            objects.extend(self.list_dependent_objects(
                schema,
                &objects,
                build_list_object_constraints_sql(false),
                ObjectKind::ObjectConstraint,
            )?);
        }
        if selection.contains(ObjectKind::RefConstraint) {
            objects.extend(self.list_dependent_objects(
                schema,
                &objects,
                build_list_object_constraints_sql(true),
                ObjectKind::RefConstraint,
            )?);
        }
        if selection.contains(ObjectKind::ObjectComment) {
            objects.extend(self.list_dependent_objects(
                schema,
                &objects,
                build_list_object_comments_sql(),
                ObjectKind::ObjectComment,
            )?);
        }
        if selection.contains(ObjectKind::ObjectGrant) {
            objects.extend(self.list_object_grants(schema, &objects)?);
        }
        Ok(objects)
    }

    pub fn get_ddl(&self, schema: &str, object: &DbObject) -> Result<String> {
        if is_dependent_metadata_kind(object.kind) {
            return self.get_dependent_ddl(schema, object);
        }

        let ddl: String = self.connection.query_row_as(
            "SELECT DBMS_METADATA.GET_DDL(:object_type, :object_name, :schema) FROM dual",
            &[&object.kind.metadata_type(), &object.name, &schema],
        )?;
        Ok(ddl)
    }

    fn get_dependent_ddl(&self, schema: &str, object: &DbObject) -> Result<String> {
        let ddl: String = self.connection.query_row_as(
            "SELECT DBMS_METADATA.GET_DEPENDENT_DDL(:object_type, :object_name, :schema) FROM dual",
            &[&object.kind.metadata_type(), &object.name, &schema],
        )?;
        Ok(ddl)
    }

    fn list_object_grants(&self, schema: &str, objects: &[DbObject]) -> Result<Vec<DbObject>> {
        let selected_objects = objects
            .iter()
            .map(|object| (object.name.as_str(), object.kind))
            .collect::<BTreeSet<_>>();
        if selected_objects.is_empty() {
            return Ok(Vec::new());
        }

        let rows = self
            .connection
            .query(build_list_object_grants_sql(), &[&schema])?;
        let mut grant_names = BTreeSet::new();
        for row_result in rows {
            let row = row_result?;
            let name: String = row.get("TABLE_NAME")?;
            let object_type: String = row.get("OBJECT_TYPE")?;
            let Some(kind) = ObjectKind::from_all_objects_type(object_type.as_str()) else {
                continue;
            };

            if selected_objects.contains(&(name.as_str(), kind)) {
                grant_names.insert(name);
            }
        }

        Ok(grant_names
            .into_iter()
            .map(|name| DbObject {
                name,
                kind: ObjectKind::ObjectGrant,
            })
            .collect())
    }

    fn list_dependent_objects(
        &self,
        schema: &str,
        objects: &[DbObject],
        sql: &str,
        kind: ObjectKind,
    ) -> Result<Vec<DbObject>> {
        let selected_names = objects
            .iter()
            .filter(|object| is_dependent_metadata_base_kind(object.kind))
            .map(|object| object.name.as_str())
            .collect::<BTreeSet<_>>();
        if selected_names.is_empty() {
            return Ok(Vec::new());
        }

        let rows = self.connection.query(sql, &[&schema])?;
        let mut names = BTreeSet::new();
        for row_result in rows {
            let row = row_result?;
            let name: String = row.get("TABLE_NAME")?;
            if selected_names.contains(name.as_str()) {
                names.insert(name);
            }
        }

        Ok(names
            .into_iter()
            .map(|name| DbObject { name, kind })
            .collect())
    }

    fn find_schema(&self, schema: &str) -> Result<Option<String>> {
        let mut rows = self.connection.query(
            "SELECT username FROM all_users WHERE username = :schema",
            &[&schema],
        )?;
        if let Some(row_result) = rows.next() {
            let row = row_result?;
            Ok(Some(row.get("USERNAME")?))
        } else {
            Ok(None)
        }
    }

    fn probe_query_available(&self, sql: &str) -> Result<bool> {
        match self.connection.query(sql, &[]) {
            Ok(_) => Ok(true),
            Err(error) if is_missing_or_forbidden(&error) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

fn configure_metadata(
    connection: &Connection,
    options: MetadataOptions,
) -> Result<Vec<MetadataTransformResult>> {
    let mut results = Vec::new();

    if !options.lossless {
        results.push(set_transform_param(
            connection,
            "SEGMENT_ATTRIBUTES",
            false,
        )?);
    }
    results.push(set_transform_param(connection, "SQLTERMINATOR", true)?);
    results.push(set_transform_param(connection, "PRETTY", true)?);
    if !options.lossless {
        results.push(set_transform_param(connection, "EMIT_SCHEMA", false)?);
    }

    Ok(results)
}

fn set_transform_param(
    connection: &Connection,
    name: &'static str,
    value: bool,
) -> Result<MetadataTransformResult> {
    let value_sql = if value { "TRUE" } else { "FALSE" };
    let sql = format!(
        "BEGIN DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, '{name}', {value_sql}); END;"
    );

    match connection.execute(sql.as_str(), &[]) {
        Ok(_) => Ok(MetadataTransformResult {
            name,
            applied: true,
            error_code: None,
            error_message: None,
        }),
        Err(error) if is_unsupported_metadata_transform(&error) => {
            let error_code = oracle_error_code(&error);
            let error_message = error
                .db_error()
                .map(|db_error| db_error.message().to_string());
            Ok(MetadataTransformResult {
                name,
                applied: false,
                error_code,
                error_message,
            })
        }
        Err(error) => Err(error.into()),
    }
}

fn identity_sequence_predicate(has_all_tab_identity_cols: bool) -> String {
    if has_all_tab_identity_cols {
        r#"(
                o.object_type != 'SEQUENCE'
                OR (
                  o.object_name NOT LIKE 'ISEQ$$\_%' ESCAPE '\'
                  AND NOT EXISTS (
                    SELECT 1
                    FROM all_tab_identity_cols identity_cols
                    WHERE identity_cols.owner = o.owner
                      AND identity_cols.sequence_name = o.object_name
                  )
                )
              )"#
        .to_string()
    } else {
        r#"(
                o.object_type != 'SEQUENCE'
                OR o.object_name NOT LIKE 'ISEQ$$\_%' ESCAPE '\'
              )"#
        .to_string()
    }
}

fn add_selector(kinds: &mut BTreeSet<ObjectKind>, selector: &str) -> Result<()> {
    for token in selector_tokens(selector) {
        match token.as_str() {
            "all" => kinds.extend(ALL_OBJECT_KINDS.iter().copied()),
            "core" => kinds.extend(CORE_OBJECT_KINDS.iter().copied()),
            _ => kinds.extend(selector_kinds(&token)?.iter().copied()),
        }
    }
    Ok(())
}

fn remove_selector(kinds: &mut BTreeSet<ObjectKind>, selector: &str) -> Result<()> {
    for token in selector_tokens(selector) {
        match token.as_str() {
            "all" => kinds.clear(),
            "core" => {
                for kind in CORE_OBJECT_KINDS {
                    kinds.remove(kind);
                }
            }
            _ => {
                for kind in selector_kinds(&token)? {
                    kinds.remove(kind);
                }
            }
        }
    }
    Ok(())
}

fn selector_tokens(selector: &str) -> Vec<String> {
    selector
        .split(',')
        .map(|value| value.trim().to_ascii_lowercase().replace('_', "-"))
        .filter(|value| !value.is_empty())
        .collect()
}

fn selector_kinds(selector: &str) -> Result<&'static [ObjectKind]> {
    let kinds = match selector {
        "table" | "tables" => &[ObjectKind::Table][..],
        "view" | "views" => &[ObjectKind::View],
        "procedure" | "procedures" => &[ObjectKind::Procedure],
        "function" | "functions" => &[ObjectKind::Function],
        "package" | "packages" => PACKAGE_KINDS,
        "package-spec" | "package-specs" => &[ObjectKind::PackageSpec],
        "package-body" | "package-bodies" => &[ObjectKind::PackageBody],
        "trigger" | "triggers" => &[ObjectKind::Trigger],
        "sequence" | "sequences" => &[ObjectKind::Sequence],
        "type" | "types" => TYPE_KINDS,
        "type-spec" | "type-specs" => &[ObjectKind::TypeSpec],
        "type-body" | "type-bodies" => &[ObjectKind::TypeBody],
        "index" | "indexes" | "indices" => &[ObjectKind::Index],
        "synonym" | "synonyms" => &[ObjectKind::Synonym],
        "grant" | "grants" | "object-grant" | "object-grants" => &[ObjectKind::ObjectGrant],
        "constraint" | "constraints" => &[ObjectKind::ObjectConstraint, ObjectKind::RefConstraint],
        "check-constraint" | "check-constraints" | "table-constraint" | "table-constraints" => {
            &[ObjectKind::ObjectConstraint]
        }
        "ref-constraint" | "ref-constraints" | "foreign-key" | "foreign-keys" => {
            &[ObjectKind::RefConstraint]
        }
        "comment" | "comments" => &[ObjectKind::ObjectComment],
        "materialized-view" | "materialized-views" | "mview" | "mviews" => {
            &[ObjectKind::MaterializedView]
        }
        "database-link" | "database-links" | "db-link" | "db-links" | "dblink" | "dblinks" => {
            &[ObjectKind::DatabaseLink]
        }
        "" => EMPTY_KINDS,
        _ => return Err(Error::InvalidObjectSelector(selector.to_string())),
    };
    Ok(kinds)
}

fn schema_lookup_value(schema: &str) -> (String, bool) {
    let trimmed = schema.trim();
    if let Some(unquoted) = unquote_schema_identifier(trimmed) {
        (unquoted, true)
    } else {
        (schema.to_string(), false)
    }
}

fn unquote_schema_identifier(schema: &str) -> Option<String> {
    if schema.len() < 2 || !schema.starts_with('"') || !schema.ends_with('"') {
        return None;
    }

    Some(schema[1..schema.len() - 1].replace("\"\"", "\""))
}

fn is_missing_or_forbidden(error: &oracle::Error) -> bool {
    matches!(oracle_error_code(error), Some(942 | 1031))
}

fn is_unsupported_metadata_transform(error: &oracle::Error) -> bool {
    matches!(
        oracle_error_code(error),
        Some(31600 | 31601 | 31602 | 31604 | 31605)
    )
}

fn oracle_error_code(error: &oracle::Error) -> Option<i32> {
    error.db_error().map(|db_error| db_error.code())
}

fn is_dependent_metadata_kind(kind: ObjectKind) -> bool {
    matches!(
        kind,
        ObjectKind::ObjectConstraint
            | ObjectKind::RefConstraint
            | ObjectKind::ObjectComment
            | ObjectKind::ObjectGrant
    )
}

fn is_dependent_metadata_base_kind(kind: ObjectKind) -> bool {
    matches!(
        kind,
        ObjectKind::Table | ObjectKind::View | ObjectKind::MaterializedView
    )
}

#[cfg(test)]
mod tests {
    use super::{
        DbCapabilities, ObjectKind, ObjectSelection, build_list_object_comments_sql,
        build_list_object_constraints_sql, build_list_object_grants_sql, build_list_objects_sql,
        schema_lookup_value,
    };

    #[test]
    fn maps_oracle_object_types_to_metadata_types() {
        assert_eq!(
            ObjectKind::from_all_objects_type("PACKAGE")
                .unwrap()
                .metadata_type(),
            "PACKAGE_SPEC"
        );
        assert_eq!(
            ObjectKind::from_all_objects_type("PACKAGE BODY")
                .unwrap()
                .metadata_type(),
            "PACKAGE_BODY"
        );
        assert_eq!(
            ObjectKind::from_all_objects_type("TABLE")
                .unwrap()
                .output_dir(),
            "TABLE"
        );
    }

    #[test]
    fn maps_all_supported_oracle_object_types_to_metadata_types_and_output_dirs() {
        let mappings = [
            ("TABLE", ObjectKind::Table, "TABLE"),
            ("VIEW", ObjectKind::View, "VIEW"),
            ("PROCEDURE", ObjectKind::Procedure, "PROCEDURE"),
            ("FUNCTION", ObjectKind::Function, "FUNCTION"),
            ("PACKAGE", ObjectKind::PackageSpec, "PACKAGE_SPEC"),
            ("PACKAGE BODY", ObjectKind::PackageBody, "PACKAGE_BODY"),
            ("TRIGGER", ObjectKind::Trigger, "TRIGGER"),
            ("SEQUENCE", ObjectKind::Sequence, "SEQUENCE"),
            ("TYPE", ObjectKind::TypeSpec, "TYPE_SPEC"),
            ("TYPE BODY", ObjectKind::TypeBody, "TYPE_BODY"),
            ("INDEX", ObjectKind::Index, "INDEX"),
            ("SYNONYM", ObjectKind::Synonym, "SYNONYM"),
            (
                "MATERIALIZED VIEW",
                ObjectKind::MaterializedView,
                "MATERIALIZED_VIEW",
            ),
            ("DATABASE LINK", ObjectKind::DatabaseLink, "DB_LINK"),
        ];

        for (all_objects_type, expected_kind, expected_metadata_type) in mappings {
            let kind = ObjectKind::from_all_objects_type(all_objects_type);

            assert_eq!(kind, Some(expected_kind));
            assert_eq!(expected_kind.metadata_type(), expected_metadata_type);
            assert_eq!(expected_kind.output_dir(), expected_metadata_type);
        }
        assert_eq!(ObjectKind::ObjectConstraint.metadata_type(), "CONSTRAINT");
        assert_eq!(ObjectKind::ObjectConstraint.output_dir(), "CONSTRAINT");
        assert_eq!(ObjectKind::RefConstraint.metadata_type(), "REF_CONSTRAINT");
        assert_eq!(ObjectKind::RefConstraint.output_dir(), "REF_CONSTRAINT");
        assert_eq!(ObjectKind::ObjectComment.metadata_type(), "COMMENT");
        assert_eq!(ObjectKind::ObjectComment.output_dir(), "COMMENT");
        assert_eq!(ObjectKind::ObjectGrant.metadata_type(), "OBJECT_GRANT");
        assert_eq!(ObjectKind::ObjectGrant.output_dir(), "OBJECT_GRANT");
    }

    #[test]
    fn rejects_unsupported_or_differently_cased_object_types() {
        for object_type in [
            "",
            "CONSTRAINT",
            "COMMENT",
            "REF_CONSTRAINT",
            "OBJECT_GRANT",
            "table",
            "Package Body",
        ] {
            assert_eq!(ObjectKind::from_all_objects_type(object_type), None);
        }
    }

    #[test]
    fn default_selection_keeps_existing_core_object_set() {
        let selection = ObjectSelection::default();

        for kind in [
            ObjectKind::Table,
            ObjectKind::View,
            ObjectKind::Procedure,
            ObjectKind::Function,
            ObjectKind::PackageSpec,
            ObjectKind::PackageBody,
            ObjectKind::Trigger,
            ObjectKind::Sequence,
            ObjectKind::TypeSpec,
            ObjectKind::TypeBody,
        ] {
            assert!(selection.contains(kind));
        }
        assert!(!selection.contains(ObjectKind::Index));
        assert!(!selection.contains(ObjectKind::Synonym));
        assert!(!selection.contains(ObjectKind::MaterializedView));
        assert!(!selection.contains(ObjectKind::DatabaseLink));
        assert!(!selection.contains(ObjectKind::ObjectConstraint));
        assert!(!selection.contains(ObjectKind::RefConstraint));
        assert!(!selection.contains(ObjectKind::ObjectComment));
        assert!(!selection.contains(ObjectKind::ObjectGrant));
    }

    #[test]
    fn include_and_exclude_selectors_are_applied_deterministically() {
        let selection = ObjectSelection::from_selectors(
            &[
                "indexes,synonyms".to_string(),
                "mviews,grants,constraints,comments".to_string(),
            ],
            &["trigger,type-body,ref-constraints".to_string()],
        )
        .unwrap();

        assert!(selection.contains(ObjectKind::Index));
        assert!(selection.contains(ObjectKind::Synonym));
        assert!(selection.contains(ObjectKind::MaterializedView));
        assert!(selection.contains(ObjectKind::ObjectConstraint));
        assert!(selection.contains(ObjectKind::ObjectComment));
        assert!(selection.contains(ObjectKind::ObjectGrant));
        assert!(!selection.contains(ObjectKind::Trigger));
        assert!(!selection.contains(ObjectKind::TypeBody));
        assert!(!selection.contains(ObjectKind::RefConstraint));
    }

    #[test]
    fn rejects_unknown_object_selector() {
        assert!(ObjectSelection::from_selectors(&["unknown".to_string()], &[]).is_err());
    }

    #[test]
    fn list_objects_sql_avoids_12c_identity_view_when_unavailable() {
        let capabilities = DbCapabilities::default();
        let sql = build_list_objects_sql(&capabilities, &ObjectSelection::default());

        assert!(sql.contains("FROM all_objects o"));
        assert!(sql.contains("o.owner = :schema"));
        assert!(!sql.contains("UPPER(:schema)"));
        assert!(!sql.contains("all_tab_identity_cols"));
        assert!(sql.contains("ISEQ$$\\_%"));
    }

    #[test]
    fn list_objects_sql_uses_identity_view_when_available() {
        let capabilities = DbCapabilities {
            has_all_tab_identity_cols: true,
            ..DbCapabilities::default()
        };
        let sql = build_list_objects_sql(&capabilities, &ObjectSelection::default());

        assert!(sql.contains("all_tab_identity_cols"));
        assert!(sql.contains("identity_cols.sequence_name = o.object_name"));
    }

    #[test]
    fn list_objects_sql_filters_constraint_owned_indexes_when_included() {
        let selection = ObjectSelection::from_selectors(&["index".to_string()], &[]).unwrap();
        let sql = build_list_objects_sql(&DbCapabilities::default(), &selection);

        assert!(sql.contains("'INDEX'"));
        assert!(sql.contains("FROM all_constraints c"));
        assert!(sql.contains("c.index_name = o.object_name"));
    }

    #[test]
    fn list_objects_sql_does_not_treat_object_grants_as_all_objects_rows() {
        let selection =
            ObjectSelection::from_selectors(&["grants,constraints,comments".to_string()], &[])
                .unwrap();
        let sql = build_list_objects_sql(&DbCapabilities::default(), &selection);

        assert!(sql.contains("'TABLE'"));
        assert!(!sql.contains("'OBJECT_GRANT'"));
        assert!(!sql.contains("'CONSTRAINT'"));
        assert!(!sql.contains("'REF_CONSTRAINT'"));
        assert!(!sql.contains("'COMMENT'"));
    }

    #[test]
    fn object_grants_sql_lists_grants_on_schema_owned_supported_objects() {
        let sql = build_list_object_grants_sql();

        assert!(sql.contains("FROM all_tab_privs p"));
        assert!(sql.contains("JOIN all_objects o"));
        assert!(sql.contains("p.table_schema = :schema"));
        assert!(sql.contains("'MATERIALIZED VIEW'"));
        assert!(!sql.contains("'PACKAGE BODY'"));
        assert!(sql.contains("ORDER BY o.object_type, p.table_name"));
    }

    #[test]
    fn constraints_sql_splits_normal_and_ref_constraints() {
        let normal = build_list_object_constraints_sql(false);
        let refs = build_list_object_constraints_sql(true);

        assert!(normal.contains("FROM all_constraints"));
        assert!(normal.contains("constraint_type != 'R'"));
        assert!(normal.contains("constraint_name NOT LIKE 'BIN$%'"));
        assert!(refs.contains("constraint_type = 'R'"));
        assert!(refs.contains("ORDER BY table_name"));
    }

    #[test]
    fn comments_sql_lists_table_and_column_comments() {
        let sql = build_list_object_comments_sql();

        assert!(sql.contains("FROM all_tab_comments"));
        assert!(sql.contains("FROM all_col_comments"));
        assert!(sql.contains("comments IS NOT NULL"));
        assert!(sql.contains("ORDER BY table_name"));
    }

    #[test]
    fn schema_lookup_preserves_explicitly_quoted_schema() {
        assert_eq!(
            schema_lookup_value("\"MixedCase\""),
            ("MixedCase".to_string(), true)
        );
        assert_eq!(
            schema_lookup_value("\"A\"\"B\""),
            ("A\"B".to_string(), true)
        );
        assert_eq!(schema_lookup_value("hr"), ("hr".to_string(), false));
    }
}
