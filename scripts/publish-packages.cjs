// Publishes the @tur-ng packages under js/packages/ whose local version does not
// match the latest version on the npm registry. Auth is OIDC trusted publishing
// (no token): pnpm publish exchanges the CI's OIDC identity automatically.
//
// Per package:
//   - local > registry  (or never published) -> publish
//   - local == registry                          -> skip
//   - local < registry                           -> warn + skip (publish would 403 anyway)
//
// Usage: node scripts/publish-packages.cjs [--dry-run]

const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const dryRun = process.argv.includes('--dry-run');
const rootDir = path.join(__dirname, '..');
const packagesDir = path.join(rootDir, 'js', 'packages');

function discoverPackages() {
  const packages = [];
  for (const entry of fs.readdirSync(packagesDir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const manifestPath = path.join(packagesDir, entry.name, 'package.json');
    if (!fs.existsSync(manifestPath)) continue;
    const manifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
    if (!manifest.publishConfig || manifest.private) continue;
    const workspaceDeps = new Set();
    for (const field of ['dependencies', 'devDependencies', 'peerDependencies']) {
      for (const dep of Object.keys(manifest[field] || {})) workspaceDeps.add(dep);
    }
    packages.push({ name: manifest.name, version: manifest.version, dir: path.dirname(manifestPath), workspaceDeps });
  }
  return packages;
}

// Dependency-first (topological) order so workspace:* ranges are rewritten to
// versions that are already on the registry when possible.
function topoSort(packages) {
  const byName = new Map(packages.map((p) => [p.name, p]));
  const sorted = [];
  const visited = new Set();
  const visit = (pkg) => {
    if (visited.has(pkg.name)) return;
    visited.add(pkg.name);
    for (const dep of pkg.workspaceDeps) {
      if (byName.has(dep)) visit(byName.get(dep));
    }
    sorted.push(pkg);
  };
  for (const pkg of packages) visit(pkg);
  return sorted;
}

// Minimal semver compare for `x.y.z[-prerelease]` (semver.org ordering).
function compareVersions(a, b) {
  const parse = (v) => {
    const match = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/.exec(v.trim());
    if (!match) throw new Error(`unparseable semver: ${v}`);
    return { core: match.slice(1, 4).map(Number), pre: match[4] ? match[4].split('.') : null };
  };
  const pa = parse(a);
  const pb = parse(b);
  for (let i = 0; i < 3; i++) {
    if (pa.core[i] !== pb.core[i]) return pa.core[i] - pb.core[i];
  }
  if (!pa.pre && !pb.pre) return 0;
  if (!pa.pre) return 1; // release > prerelease
  if (!pb.pre) return -1;
  const len = Math.max(pa.pre.length, pb.pre.length);
  for (let i = 0; i < len; i++) {
    const x = pa.pre[i];
    const y = pb.pre[i];
    if (x === undefined) return -1;
    if (y === undefined) return 1;
    const xn = /^\d+$/.test(x);
    const yn = /^\d+$/.test(y);
    if (xn && yn) {
      if (Number(x) !== Number(y)) return Number(x) - Number(y);
    } else if (xn) {
      return -1;
    } else if (yn) {
      return 1;
    } else if (x !== y) {
      return x < y ? -1 : 1;
    }
  }
  return 0;
}

function registryVersion(name) {
  try {
    return execFileSync('npm', ['view', name, 'version'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim() || null;
  } catch (err) {
    const stderr = String(err.stderr || '');
    if (stderr.includes('E404')) return null; // never published
    throw err;
  }
}

const packages = topoSort(discoverPackages());
if (packages.length === 0) {
  console.log('No publishable packages found (expected publishConfig in js/packages/*/package.json).');
  process.exit(0);
}

const toPublish = [];
const skipped = [];
const behind = [];

for (const pkg of packages) {
  const remote = registryVersion(pkg.name);
  if (remote === null) {
    console.log(`[publish] ${pkg.name}@${pkg.version} (not yet on the registry)`);
    toPublish.push(pkg);
  } else if (compareVersions(pkg.version, remote) > 0) {
    console.log(`[publish] ${pkg.name}@${remote} -> ${pkg.version}`);
    toPublish.push(pkg);
  } else if (compareVersions(pkg.version, remote) === 0) {
    console.log(`[skip]    ${pkg.name}@${pkg.version} (matches the registry)`);
    skipped.push(pkg);
  } else {
    console.warn(`[warn]    ${pkg.name}: local ${pkg.version} is behind the registry ${remote} — skipping`);
    behind.push(pkg);
  }
}

if (toPublish.length === 0) {
  console.log('Nothing to publish.');
  process.exit(0);
}

if (dryRun) {
  console.log(`\n[dry-run] Would publish: ${toPublish.map((p) => `${p.name}@${p.version}`).join(', ')}`);
  process.exit(0);
}

for (const pkg of toPublish) {
  console.log(`\nPublishing ${pkg.name}@${pkg.version} ...`);
  execFileSync('pnpm', ['publish', '--access', 'public', '--no-git-checks'], { cwd: pkg.dir, stdio: 'inherit' });
}

console.log(`\nDone. Published ${toPublish.length}, skipped ${skipped.length}, behind-registry ${behind.length}.`);
