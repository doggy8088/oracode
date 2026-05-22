# AGENTS Instructions for oracode

These instructions apply to the entire repository.

## Project Intent

- Keep `oracode` focused on exporting stable, Git-friendly Oracle DDL.
- Preserve deterministic output and low-noise diffs as a primary goal.

## Code Areas and Expectations

- Rust core logic lives in `src/`.
- npm wrapper and install logic live in `npm/` and `tests/postinstall.test.cjs`.
- Documentation lives in `README.md` and `docs/`.

## Change Rules

- Make minimal, surgical changes that preserve existing behavior unless behavior changes are explicitly requested.
- Do not check in generated artifacts from `target/`.
- Do not modify files under `oracode-out/` unless the task explicitly asks to refresh sample output.
- Keep object-to-folder mapping semantics stable (for example `PACKAGE` -> `PACKAGE_SPEC`, `PACKAGE BODY` -> `PACKAGE_BODY`).
- Keep "unchanged content should not be rewritten" behavior intact.

## Rust Conventions

- Follow current module boundaries (`cli`, `db`, `export`, `sanitize`, `error`).
- Use typed errors via existing `Error`/`Result` patterns instead of ad-hoc panic paths.
- For concurrency or DB export changes, maintain semaphore-based throttling and clear error context per object.
- For sanitizer changes, ensure quoted identifiers, string literals, and comments are not unintentionally altered.

## Validation Checklist

- If Rust code changes: run `cargo test`.
- If CLI behavior changes: verify help or argument behavior remains coherent.
- If npm wrapper changes: run `npm test`.
- If both Rust and npm are touched: run both test suites.

## Security and Secrets

- Never hardcode database credentials, DSNs, tokens, or local absolute machine-specific secrets.
- Prefer environment variables and existing CLI/env input patterns (`ORACODE_*`).

## Documentation Updates

- Update `README.md` when user-visible CLI behavior or required setup changes.
- Update `docs/TECHNICAL.md` when architecture or core flow changes materially.
