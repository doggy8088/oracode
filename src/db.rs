use oracle::Connection;

use crate::cli::ConnectionConfig;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbObject {
    pub name: String,
    pub kind: ObjectKind,
}

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
            _ => None,
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
        }
    }

    pub fn output_dir(self) -> &'static str {
        self.metadata_type()
    }
}

pub struct OracleMetadataClient {
    connection: Connection,
}

impl OracleMetadataClient {
    pub fn connect(config: &ConnectionConfig) -> Result<Self> {
        let connection = Connection::connect(
            config.user.as_str(),
            config.password.as_str(),
            config.connect_descriptor().as_str(),
        )?;
        let client = Self { connection };
        client.configure_metadata()?;
        Ok(client)
    }

    pub fn list_objects(&self, schema: &str) -> Result<Vec<DbObject>> {
        let sql = r#"
            SELECT object_name, object_type
            FROM all_objects
            WHERE owner = UPPER(:schema)
              AND object_type IN (
                'TABLE',
                'VIEW',
                'PROCEDURE',
                'FUNCTION',
                'PACKAGE',
                'PACKAGE BODY',
                'TRIGGER',
                'SEQUENCE',
                'TYPE',
                'TYPE BODY'
              )
              AND object_name NOT LIKE 'BIN$%'
            ORDER BY object_type, object_name
        "#;

        let rows = self.connection.query(sql, &[&schema])?;
        let mut objects = Vec::new();
        for row_result in rows {
            let row = row_result?;
            let name: String = row.get("OBJECT_NAME")?;
            let object_type: String = row.get("OBJECT_TYPE")?;
            let kind = ObjectKind::from_all_objects_type(object_type.as_str())
                .ok_or_else(|| Error::UnsupportedObjectType(object_type.clone()))?;
            objects.push(DbObject { name, kind });
        }
        Ok(objects)
    }

    pub fn get_ddl(&self, schema: &str, object: &DbObject) -> Result<String> {
        let ddl: String = self.connection.query_row_as(
            "SELECT DBMS_METADATA.GET_DDL(:object_type, :object_name, UPPER(:schema)) FROM dual",
            &[&object.kind.metadata_type(), &object.name, &schema],
        )?;
        Ok(ddl)
    }

    fn configure_metadata(&self) -> Result<()> {
        self.connection.execute(
            r#"
            BEGIN
                DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'SEGMENT_ATTRIBUTES', FALSE);
                DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'SQLTERMINATOR', TRUE);
                DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'PRETTY', TRUE);
                DBMS_METADATA.SET_TRANSFORM_PARAM(DBMS_METADATA.SESSION_TRANSFORM, 'EMIT_SCHEMA', FALSE);
            END;
            "#,
            &[],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectKind;

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
}
