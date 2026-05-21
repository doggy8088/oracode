use std::collections::HashSet;
use std::sync::LazyLock;

use regex::{Captures, Regex};

static EDITIONABLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s+\b(?:NON)?EDITIONABLE\b\s+").unwrap());
static SIMPLE_QUOTED_IDENTIFIER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""([A-Z][A-Z0-9_$#]*)""#).unwrap());
static EXCESSIVE_BLANK_LINES_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
static KEYWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "ADD",
        "ALTER",
        "AND",
        "AS",
        "BEGIN",
        "BLOB",
        "BODY",
        "BOOLEAN",
        "BY",
        "CASE",
        "CHAR",
        "CHECK",
        "CLOB",
        "COMMENT",
        "CONSTRAINT",
        "CREATE",
        "DATE",
        "DEFAULT",
        "DELETE",
        "DISABLE",
        "ELSE",
        "ENABLE",
        "END",
        "FOR",
        "FOREIGN",
        "FROM",
        "FUNCTION",
        "GRANT",
        "GROUP",
        "IF",
        "IN",
        "INDEX",
        "INTEGER",
        "INSERT",
        "IS",
        "KEY",
        "LIKE",
        "LOB",
        "NCHAR",
        "NCLOB",
        "NUMBER",
        "NVARCHAR2",
        "NOT",
        "NULL",
        "ON",
        "OR",
        "ORDER",
        "PACKAGE",
        "PRIMARY",
        "PROCEDURE",
        "REFERENCES",
        "REPLACE",
        "RETURN",
        "ROWID",
        "RAW",
        "SELECT",
        "SEQUENCE",
        "STORE",
        "TABLE",
        "TABLESPACE",
        "THEN",
        "TIMESTAMP",
        "TRIGGER",
        "TYPE",
        "UNIQUE",
        "UPDATE",
        "VALUES",
        "VARCHAR",
        "VARCHAR2",
        "VIEW",
        "WHEN",
        "WHERE",
    ]
    .into_iter()
    .collect()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SanitizeOptions {
    pub keep_quotes: bool,
}

pub fn sanitize_ddl(input: &str, options: SanitizeOptions) -> String {
    let without_editionable = EDITIONABLE_RE.replace_all(input, " ");
    let unquoted = if options.keep_quotes {
        without_editionable.into_owned()
    } else {
        SIMPLE_QUOTED_IDENTIFIER_RE
            .replace_all(&without_editionable, |captures: &Captures<'_>| {
                captures[1].to_string()
            })
            .into_owned()
    };
    let normalized = normalize_keywords(&unquoted);
    let trimmed = trim_trailing_whitespace(&normalized);
    ensure_final_newline(
        EXCESSIVE_BLANK_LINES_RE
            .replace_all(&trimmed, "\n\n")
            .as_ref(),
    )
}

fn normalize_keywords(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\'' => {
                output.push(ch);
                copy_single_quoted(&mut chars, &mut output);
            }
            '"' => {
                output.push(ch);
                copy_double_quoted(&mut chars, &mut output);
            }
            '-' if chars.peek() == Some(&'-') => {
                output.push(ch);
                output.push(chars.next().unwrap());
                copy_until_newline(&mut chars, &mut output);
            }
            '/' if chars.peek() == Some(&'*') => {
                output.push(ch);
                output.push(chars.next().unwrap());
                copy_until_block_comment_end(&mut chars, &mut output);
            }
            _ if is_word_start(ch) => {
                let mut word = String::from(ch);
                while let Some(next) = chars.peek().copied() {
                    if is_word_continue(next) {
                        word.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                let upper = word.to_ascii_uppercase();
                if KEYWORDS.contains(upper.as_str()) {
                    output.push_str(&upper);
                } else {
                    output.push_str(&word);
                }
            }
            _ => output.push(ch),
        }
    }

    output
}

fn copy_single_quoted<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = char>,
{
    while let Some(ch) = chars.next() {
        output.push(ch);
        if ch == '\'' {
            if chars.peek() == Some(&'\'') {
                output.push(chars.next().unwrap());
            } else {
                break;
            }
        }
    }
}

fn copy_double_quoted<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = char>,
{
    for ch in chars.by_ref() {
        output.push(ch);
        if ch == '"' {
            break;
        }
    }
}

fn copy_until_newline<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = char>,
{
    for ch in chars.by_ref() {
        output.push(ch);
        if ch == '\n' {
            break;
        }
    }
}

fn copy_until_block_comment_end<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = char>,
{
    let mut previous = '\0';
    for ch in chars.by_ref() {
        output.push(ch);
        if previous == '*' && ch == '/' {
            break;
        }
        previous = ch;
    }
}

fn trim_trailing_whitespace(input: &str) -> String {
    input
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn ensure_final_newline(input: &str) -> String {
    let mut output = input.trim_start_matches('\u{feff}').trim().to_string();
    output.push('\n');
    output
}

fn is_word_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_word_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' || ch == '#'
}

#[cfg(test)]
mod tests {
    use super::{SanitizeOptions, sanitize_ddl};

    #[test]
    fn removes_editionable_and_simple_identifier_quotes() {
        let ddl = r#"
create or replace editionable view "EMPLOYEES" as
select "EMPLOYEE_ID", "NAME" from "HR"."EMPLOYEES";
"#;

        assert_eq!(
            sanitize_ddl(ddl, SanitizeOptions { keep_quotes: false }),
            "CREATE OR REPLACE VIEW EMPLOYEES AS\nSELECT EMPLOYEE_ID, NAME FROM HR.EMPLOYEES;\n"
        );
    }

    #[test]
    fn preserves_quotes_when_requested() {
        let ddl = r#"create table "ORDER" ("ID" number);"#;

        assert_eq!(
            sanitize_ddl(ddl, SanitizeOptions { keep_quotes: true }),
            "CREATE TABLE \"ORDER\" (\"ID\" NUMBER);\n"
        );
    }

    #[test]
    fn does_not_normalize_string_literals_or_comments() {
        let ddl =
            "create view v as select 'select from' as text from dual -- where stays lowercase\n";

        assert_eq!(
            sanitize_ddl(ddl, SanitizeOptions { keep_quotes: false }),
            "CREATE VIEW v AS SELECT 'select from' AS text FROM dual -- where stays lowercase\n"
        );
    }

    #[test]
    fn collapses_excessive_blank_lines() {
        let ddl =
            "create table t (id number);\n\n\n\nalter table t add constraint pk primary key (id);";

        assert_eq!(
            sanitize_ddl(ddl, SanitizeOptions { keep_quotes: false }),
            "CREATE TABLE t (id NUMBER);\n\nALTER TABLE t ADD CONSTRAINT pk PRIMARY KEY (id);\n"
        );
    }
}
