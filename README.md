# oracode

`oracode` exports Oracle database objects as clean, stable, Git-friendly DDL files.

## Features

- One Oracle object per `.sql` file.
- Stable output directories such as `TABLE/`, `VIEW/`, `PACKAGE_SPEC/`, and `PACKAGE_BODY/`.
- `DBMS_METADATA` transform setup for `SEGMENT_ATTRIBUTES = FALSE`, `SQLTERMINATOR = TRUE`, `PRETTY = TRUE`, and `EMIT_SCHEMA = FALSE`.
- Extra DDL cleanup for `EDITIONABLE`, redundant blank lines, simple quoted identifiers, and SQL keyword casing.
- Concurrent export with a terminal progress bar.
- Skips rewriting unchanged files.

## Install

From source:

```sh
cargo install --path .
```

After releases are published, npm users can install the thin wrapper package:

```sh
npm i -g oracode
```

## Oracle client requirements

The Rust `oracle` driver uses ODPI-C and needs Oracle Client libraries at runtime.
Install Oracle Instant Client and make the library directory discoverable:

- macOS: set `DYLD_LIBRARY_PATH` or ensure the Instant Client directory is on the loader path.
- Linux: set `LD_LIBRARY_PATH=/path/to/instantclient`.
- Windows: add the Instant Client directory to `PATH`.

## Usage

```sh
oracode \
  --host db.example.com \
  --port 1521 \
  --user HR \
  --password "$ORACLE_PASSWORD" \
  --service-name ORCLPDB1 \
  --schema HR \
  --output ./oracode-out
```

Use `--sid XE` instead of `--service-name` for SID-based connections.
Use `--keep-quotes` when quoted identifiers must remain quoted.

All CLI options can also be supplied with `ORACODE_*` environment variables, for example `ORACODE_PASSWORD`.
