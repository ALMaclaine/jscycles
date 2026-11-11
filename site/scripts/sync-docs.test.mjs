import { test } from 'node:test';
import assert from 'node:assert/strict';
import { anchor, rewriteLink, splitTitle, usageBlock } from './sync-docs.mjs';

const titles = { 'analysis-limits.md': 'Analysis limits', 'configuration.md': 'Configuration' };

test('links between synced docs become site links', () => {
  assert.equal(rewriteLink('configuration.md', 'docs/analysis-limits.md'), '../configuration/');
  assert.equal(
    rewriteLink('configuration.md#type-only-imports', 'docs/analysis-limits.md'),
    '../configuration/#type-only-imports',
  );
  assert.equal(rewriteLink('migration-0.2.md', 'docs/analysis-limits.md'), '../migration-0-2/');
});

test('an anchor naming the target page title links to the page', () => {
  assert.equal(
    rewriteLink('analysis-limits.md#analysis-limits', 'docs/configuration.md', titles),
    '../analysis-limits/',
  );
});

test('other repository files link to GitHub; URLs and anchors are kept', () => {
  assert.equal(
    rewriteLink('../CHANGELOG.md', 'docs/migration-0.2.md'),
    'https://github.com/ALMaclaine/jscycles/blob/main/CHANGELOG.md',
  );
  assert.equal(
    rewriteLink('maintainers/build-trust.md#x', 'docs/README.md'),
    'https://github.com/ALMaclaine/jscycles/blob/main/docs/maintainers/build-trust.md#x',
  );
  assert.equal(rewriteLink('https://example.com/a.md'), 'https://example.com/a.md');
  assert.equal(rewriteLink('#local'), '#local');
});

test('the leading title is required and removed', () => {
  assert.deepEqual(splitTitle('\uFEFF# Package cycles\n\nBody.\n', 'x.md'), {
    title: 'Package cycles',
    body: 'Body.\n',
  });
  assert.throws(() => splitTitle('No title\n', 'x.md'), /must start with/);
});

test('anchors follow GitHub heading slugs', () => {
  assert.equal(anchor('Analysis limits'), 'analysis-limits');
  assert.equal(anchor('`defaults` and `packages`'), 'defaults-and-packages');
  assert.equal(anchor('Upgrading to 0.2'), 'upgrading-to-02');
});

test('the usage block is read from the README', () => {
  assert.equal(usageBlock('x\n```text\nUsage: jscycles [OPTIONS]\n```\n'), 'Usage: jscycles [OPTIONS]\n');
  assert.throws(() => usageBlock('no usage'), /Usage: jscycles/);
});
