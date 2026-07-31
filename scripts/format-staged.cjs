#!/usr/bin/env node
// Formats staged files with rustfmt (Rust) and biome (JS/TS/JSON/CSS),
// then re-stages anything the formatters touched. Non-fatal: formatter
// errors are reported as warnings so they never block a commit (CI enforces
// lint/clippy). Invoked by the git-end agent right before it commits.

const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const rootDir = path.join(__dirname, '..');
const biomeBin = path.join('js', 'node_modules', '.bin', 'biome');

// Biome scope, matching the `pnpm format`/`pnpm lint` scripts in js/package.json.
const biomeScopes = ['js/', 'demo/website/', 'demo/playground-view/'];
const biomeExts = new Set([
  '.js', '.jsx', '.mjs', '.cjs', '.ts', '.tsx', '.mts', '.cts',
  '.json', '.jsonc', '.css',
]);

function run(cmd) {
  return execSync(cmd, { cwd: rootDir, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
}

function quote(p) {
  return `"${p.replace(/"/g, '\\"')}"`;
}

// Collect staged (added/copied/modified/renamed) files.
let staged;
try {
  staged = run('git diff --cached --name-only --diff-filter=ACMR').split('\n').filter(Boolean);
} catch {
  console.log('format-staged: not in a git repo or git unavailable; skipping.');
  process.exit(0);
}

if (staged.length === 0) {
  console.log('format-staged: no staged files; skipping.');
  process.exit(0);
}

const rustFiles = staged.filter((f) => f.endsWith('.rs'));
const biomeFiles = staged.filter((f) => {
  if (!biomeExts.has(path.extname(f))) return false;
  const norm = f.replace(/\\/g, '/');
  return biomeScopes.some((s) => norm.startsWith(s));
});

// Only re-stage files that actually exist on disk (guards against quirky states).
const exists = (f) => fs.existsSync(path.join(rootDir, f));

const rustTargets = rustFiles.filter(exists);
const biomeTargets = biomeFiles.filter(exists);
const touched = [];

if (rustTargets.length > 0) {
  try {
    execSync(`rustfmt --edition 2024 ${rustTargets.map(quote).join(' ')}`, {
      cwd: rootDir,
      stdio: 'ignore',
    });
    touched.push(...rustTargets);
  } catch {
    console.error(`format-staged: rustfmt failed on ${rustTargets.length} Rust file(s); leaving as-is.`);
  }
}

if (biomeTargets.length > 0) {
  try {
    // `check --write` applies formatting + safe lint/import fixes; a non-zero
    // exit (unfixable lint) is non-fatal — formatting is still written.
    execSync(`${quote(biomeBin)} check --write ${biomeTargets.map(quote).join(' ')}`, {
      cwd: rootDir,
      stdio: 'ignore',
    });
    touched.push(...biomeTargets);
  } catch {
    console.error(`format-staged: biome reported unfixable issues on ${biomeTargets.length} file(s); formatting applied where possible.`);
    touched.push(...biomeTargets);
  }
}

if (touched.length > 0) {
  const unique = [...new Set(touched)];
  try {
    run(`git add -- ${unique.map(quote).join(' ')}`);
    console.log(`format-staged: formatted & re-staged ${unique.length} file(s) (${rustTargets.length} rust, ${biomeTargets.length} js/ts).`);
  } catch (err) {
    console.error('format-staged: failed to re-stage formatted files:', err.message);
  }
} else {
  console.log('format-staged: nothing to format.');
}
