'use strict';

const assert = require('node:assert/strict');
const { mkdirSync, mkdtempSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const test = require('node:test');

const {
  artifactName,
  cargoTarget,
  findExtractedBinary,
  githubRepo,
  platformKey,
  releaseBaseUrl,
  sha256,
  verifyChecksum,
} = require('../npm/postinstall.cjs');

test('maps supported platforms to Rust targets', () => {
  assert.equal(platformKey('darwin', 'arm64'), 'darwin-arm64');
  assert.equal(cargoTarget('darwin', 'arm64'), 'aarch64-apple-darwin');
  assert.equal(cargoTarget('darwin', 'x64'), 'x86_64-apple-darwin');
  assert.equal(cargoTarget('linux', 'x64'), 'x86_64-unknown-linux-gnu');
  assert.equal(cargoTarget('win32', 'x64'), 'x86_64-pc-windows-msvc');
});

test('rejects unsupported platforms', () => {
  assert.throws(() => cargoTarget('linux', 'arm'), /Unsupported platform/);
});

test('formats artifact names and release URLs', () => {
  assert.equal(artifactName('x86_64-unknown-linux-gnu'), 'oracode-x86_64-unknown-linux-gnu.tar.xz');
  assert.equal(artifactName('x86_64-pc-windows-msvc'), 'oracode-x86_64-pc-windows-msvc.zip');
  assert.equal(githubRepo(), 'doggy8088/oracode');
  assert.equal(releaseBaseUrl('1.2.3'), 'https://github.com/doggy8088/oracode/releases/download/v1.2.3');
});

test('verifies sha256 checksums', () => {
  const dir = mkdtempSync(join(tmpdir(), 'oracode-'));
  const file = join(dir, 'sample.txt');
  writeFileSync(file, 'hello');
  const digest = sha256(file);
  verifyChecksum(file, `${digest}  sample.txt`);
  assert.throws(() => verifyChecksum(file, '0'.repeat(64)), /Checksum mismatch/);
});

test('githubRepo honors environment override', () => {
  const previous = process.env.ORACODE_GITHUB_REPO;
  process.env.ORACODE_GITHUB_REPO = 'example/custom-oracode';
  try {
    assert.equal(githubRepo(), 'example/custom-oracode');
    assert.equal(releaseBaseUrl('9.8.7'), 'https://github.com/example/custom-oracode/releases/download/v9.8.7');
  } finally {
    if (previous === undefined) {
      delete process.env.ORACODE_GITHUB_REPO;
    } else {
      process.env.ORACODE_GITHUB_REPO = previous;
    }
  }
});

test('rejects malformed checksum files', () => {
  const dir = mkdtempSync(join(tmpdir(), 'oracode-'));
  const file = join(dir, 'sample.txt');
  writeFileSync(file, 'hello');

  assert.throws(() => verifyChecksum(file, 'not-a-sha sample.txt'), /Invalid checksum file format/);
  assert.throws(() => verifyChecksum(file, ''), /Invalid checksum file format/);
});

test('finds extracted binary directly in archive directory', () => {
  const dir = mkdtempSync(join(tmpdir(), 'oracode-'));
  const binary = join(dir, 'oracode-test-bin');
  writeFileSync(binary, '');

  assert.equal(findExtractedBinary(dir, 'oracode-test-bin'), binary);
});

test('finds extracted binary one directory deep', () => {
  const dir = mkdtempSync(join(tmpdir(), 'oracode-'));
  const nested = join(dir, 'release');
  mkdirSync(nested);
  const binary = join(nested, 'oracode-test-bin');
  writeFileSync(binary, '');

  assert.equal(findExtractedBinary(dir, 'oracode-test-bin'), binary);
});

test('throws when extracted archive does not contain binary', () => {
  const dir = mkdtempSync(join(tmpdir(), 'oracode-'));

  assert.throws(() => findExtractedBinary(dir, 'missing-bin'), /Archive did not contain missing-bin/);
});
