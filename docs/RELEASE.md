# Release Process

This project releases prebuilt `oracode` binaries through GitHub Releases and publishes the npm wrapper package from the published GitHub Release.

## Prerequisites

- Push access to `doggy8088/oracode`.
- A clean `main` branch with CI passing.
- npm package publish access for `oracode`.
- GitHub Actions must have permission to create releases and publish npm packages with provenance.

## Versioning

Use semantic versioning and tag releases as `vMAJOR.MINOR.PATCH`, for example `v0.1.0`.

Keep these versions aligned before tagging:

- `Cargo.toml` package version.
- `package.json` version.
- `Cargo.lock`, after updating the Cargo package version.

## Pre-release checks

Run the same checks as CI:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
npm test
```

Optionally check the local optimized binary:

```sh
cargo build --release --locked
```

## Create a release

1. Update versions in `Cargo.toml` and `package.json`.
2. Refresh the lockfile if needed:

   ```sh
   cargo check
   ```

3. Commit the version update:

   ```sh
   git add Cargo.toml Cargo.lock package.json
   git commit -m "Release vX.Y.Z"
   ```

4. Create and push the release tag:

   ```sh
   git tag vX.Y.Z
   git push origin main
   git push origin vX.Y.Z
   ```

Pushing a `v*.*.*` tag starts `.github/workflows/release.yml`. The workflow builds release binaries for:

- `aarch64-apple-darwin`
- `x86_64-apple-darwin`
- `x86_64-unknown-linux-gnu`
- `x86_64-pc-windows-msvc`

It uploads each archive and its `.sha256` file to the matching GitHub Release.

## npm publishing

Publishing the GitHub Release starts `.github/workflows/npm-publish.yml`.

That workflow:

1. Aligns the npm package version with the release tag.
2. Verifies that all expected GitHub Release assets are available.
3. Publishes the npm wrapper with provenance:

   ```sh
   npm publish --provenance --access public
   ```

Do not publish npm before the GitHub Release assets exist. The package `postinstall` downloads the platform-specific binary from the GitHub Release for the package version.

## Post-release verification

After both workflows finish:

```sh
gh release view vX.Y.Z
npm view @willh/oracode version
npm i -g @willh/oracode
oracode --help
```

Also verify that each release asset has a matching `.sha256` file.

## Manual recovery

If GitHub Release asset upload fails, rerun the Release workflow for the tag or use:

```sh
gh release upload vX.Y.Z artifacts/* --clobber
```

If npm publishing fails after the GitHub Release is complete, rerun the Publish npm workflow. The workflow includes retries for asset availability because GitHub Release assets can take a short time to become downloadable.
