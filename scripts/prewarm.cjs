const { execSync } = require('child_process');
const path = require('path');

const root = path.join(__dirname, '..');
const wasmDir = path.join(root, 'demo', 'website', 'native');

function run(label, cmd, opts) {
  console.log(`[prewarm] ${label}...`);
  execSync(cmd, { stdio: 'inherit', ...opts });
}

run('pnpm install', 'pnpm install --frozen-lockfile', { cwd: root });
run('build js', 'pnpm build', { cwd: root });
// Same profile as .github/workflows/local-ci.yml (retries = 2) — the image
// build must not be stricter than CI itself over a known-flaky scheduling test.
run('cargo nextest', 'xvfb-run -a cargo nextest run --workspace --locked --profile ci', { cwd: root });
run('wasm-pack build', 'wasm-pack build --target web --no-opt -- --locked', { cwd: wasmDir });
