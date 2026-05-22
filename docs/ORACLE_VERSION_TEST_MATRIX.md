# Oracle version test matrix and fixtures

This plan keeps cross-version compatibility testable without requiring Oracle
CI credentials today. The reusable SQL fixtures live under
`tests/fixtures/oracle/` and are intended for manual or future self-hosted
snapshot export runs.

## Test tiers

| Tier | Runs where | Purpose | Current status |
| --- | --- | --- | --- |
| Tier 0: no database | GitHub-hosted CI and local dev | Rust/npm unit tests plus static review of fixture SQL and docs | Automated today; does not prove Oracle server compatibility |
| Tier 1: current XE smoke | Developer machine or future opt-in workflow | Load common fixtures into a disposable 21c XE/Free-style database and export snapshots | Manual today; no repository secrets required |
| Tier 2: maintained versions | Self-hosted runner or manually provisioned databases | Scheduled snapshot exports for 19c, 21c, and 23ai using real client/server pairs | Planned; requires privately managed databases/secrets |
| Tier 3: legacy versions | Manual lab only | Prove discovery SQL and DBMS_METADATA transforms remain safe on 9i/10g, 11g, and 12c | Manual because public CI images and modern clients are limited |

## Version matrix

| Oracle server | Recommended tier | Fixture set | Export focus | Client compatibility caveats |
| --- | --- | --- | --- | --- |
| 9i / 10g | Tier 3 manual | `common_core.sql`; optionally `optional_mviews_synonyms.sql` if privileges exist | Pre-12c discovery must not require `ALL_TAB_IDENTITY_COLS`; core object folders, indexes, constraints, comments, synonyms, and materialized views | Use Oracle's client/server interoperability matrix as authoritative. Modern Instant Client releases may not connect to 9i/10g, so keep a compatible legacy client host available. Some DBMS_METADATA transforms may be ignored by best-effort setup. |
| 11g | Tier 3 manual or self-hosted | `common_core.sql`, `optional_11g_compound_trigger.sql`, optional mviews/synonyms/grants | Same as 9i/10g plus 11g PL/SQL syntax; no identity-column sequence anti-join required | 11.2 is commonly easier to pair with newer clients than 11.1, but verify the exact client/server support combination before testing. |
| 12c | Tier 3 manual or self-hosted | Common set plus `optional_12c_identity.sql` | Identity table should export as a table while generated `ISEQ$$_...` sequences stay out of `SEQUENCE/`; PDB service-name connections | Prefer service names for PDBs. Use an Instant Client version explicitly supported for the target 12c patch level. |
| 18c | Tier 2 manual/self-hosted | Common set plus 12c identity fixture | Regression guard for post-12c DBMS_METADATA differences and unchanged-file behavior | Use a supported 18c/19c-era client; verify server patch support status. |
| 19c | Tier 2 recommended baseline | Common set plus 12c identity fixture; optional grants/mviews/synonyms | Long-term-support baseline for maintained compatibility snapshots | 19c Instant Client is a practical default for many maintained databases, but it is not a universal legacy-client substitute. |
| 21c | Tier 1 local smoke and Tier 2 | Common set, 12c identity, `optional_21c_json.sql`, optional mviews/synonyms/grants | Current local Docker XE smoke path; validates newer metadata output and optional JSON datatype | Existing `make oracle-up` uses a 21c XE image. Keep `LD_LIBRARY_PATH`/`DYLD_LIBRARY_PATH`/`PATH` pointed at the matching local Instant Client. |
| 23ai | Tier 2 manual/self-hosted | Common set, 12c identity, 21c JSON, `optional_23ai_boolean.sql` | Latest-version forward compatibility, especially new SQL datatypes | Use a 23ai/Free database and a client version that Oracle documents as compatible with that server. Treat output changes as expected until reviewed. |

## Fixture loading

Use a disposable schema such as `ORACODE_FIXTURE`. A DBA can grant equivalent
object-creation privileges; exact grant syntax differs by database generation.
A typical maintained-version schema needs create session/table/view/sequence/
procedure/type/trigger/synonym privileges, quota on the default tablespace, and
optional create materialized view or create database link privileges for the
optional fixtures.

Load only fixtures that the target version supports:

```sh
sqlplus ORACODE_FIXTURE/"$ORACLE_PASSWORD"@//localhost:1521/XE @tests/fixtures/oracle/common_core.sql
sqlplus ORACODE_FIXTURE/"$ORACLE_PASSWORD"@//localhost:1521/XE @tests/fixtures/oracle/optional_mviews_synonyms.sql
sqlplus ORACODE_FIXTURE/"$ORACLE_PASSWORD"@//localhost:1521/XE @tests/fixtures/oracle/optional_12c_identity.sql
```

For grant testing, define a real grantee first:

```sql
DEFINE ORACODE_GRANTEE = ORACODE_READONLY
@tests/fixtures/oracle/optional_grants.sql
```

`optional_db_link_template.sql` is intentionally a template because database
links require environment-specific remote credentials and may produce masked or
sensitive metadata.

## Snapshot export recipe

Keep generated output under ignored `oracle-test-runs/` directories, not in the
tracked sample output.

```sh
RUN_ROOT=oracle-test-runs/21c-core
rm -rf "$RUN_ROOT"
mkdir -p "$RUN_ROOT"

oracode \
  --host localhost \
  --port 1521 \
  --user ORACODE_FIXTURE \
  --password "$ORACLE_PASSWORD" \
  --service-name XE \
  --schema ORACODE_FIXTURE \
  --output "$RUN_ROOT/snapshot-1" \
  --include indexes,constraints,comments,synonyms,mviews,grants \
  --concurrency 4

oracode ... --output "$RUN_ROOT/snapshot-2" --include indexes,constraints,comments,synonyms,mviews,grants

git --no-pager diff --no-index -- "$RUN_ROOT/snapshot-1" "$RUN_ROOT/snapshot-2"
```

Expected checks:

- Same-version snapshot directories should be identical after reloading the same
  fixture set.
- Cross-version diffs should be reviewed and documented; stable sanitizer
  changes should reduce noise but cannot eliminate all DBMS_METADATA differences.
- The object-to-folder mapping must remain stable, especially `PACKAGE` to
  `PACKAGE_SPEC/` and `PACKAGE BODY` to `PACKAGE_BODY/`.
- With `--lossless`, repeat the same snapshot comparison separately; do not mix
  lossy and lossless outputs in the same baseline.

## Unchanged-file behavior

To verify that unchanged content is not rewritten, export twice to the same
output directory. The second run should report `0 written` and all objects as
`unchanged`.

For a stricter mtime check:

```sh
OUT=oracle-test-runs/21c-core/stable-out
oracode ... --output "$OUT" --include indexes,constraints,comments,synonyms,mviews,grants
ORACODE_OUT="$OUT" python3 - <<'PY' > oracle-test-runs/mtimes-before.txt
import os
from pathlib import Path
for path in sorted(Path(os.environ['ORACODE_OUT']).rglob('*.sql')):
    print(f'{path}\t{path.stat().st_mtime_ns}')
PY
oracode ... --output "$OUT" --include indexes,constraints,comments,synonyms,mviews,grants
ORACODE_OUT="$OUT" python3 - <<'PY' > oracle-test-runs/mtimes-after.txt
import os
from pathlib import Path
for path in sorted(Path(os.environ['ORACODE_OUT']).rglob('*.sql')):
    print(f'{path}\t{path.stat().st_mtime_ns}')
PY
diff -u oracle-test-runs/mtimes-before.txt oracle-test-runs/mtimes-after.txt
```

## Promotion path

1. Keep Tier 0 in normal CI.
2. Add an opt-in local smoke target only after a fixture schema bootstrap command
   is reliable across developer machines.
3. Add self-hosted scheduled jobs for 19c/21c/23ai when private databases and
   secrets are available.
4. Record 9i/10g/11g/12c manual runs in release notes when compatibility issues
   are fixed.
