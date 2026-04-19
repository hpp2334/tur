const { execSync } = require('child_process');
const path = require('path');

const root = path.join(__dirname, '..');
const jsDir = path.join(root, 'js');
const wasmDir = path.join(root, 'libs', 'tur-wasm');

function run(label, cmd, opts) {
  console.log(`[prewarm] ${label}...`);
  execSync(cmd, { stdio: 'inherit', ...opts });
}

run('pnpm install', 'pnpm install --frozen-lockfile', { cwd: jsDir });
run('gen types', 'cargo test -p tur-shared --lib export_bindings --locked', { cwd: root });
run('build js', 'pnpm build', { cwd: jsDir });
run('cargo test', 'cargo test --workspace --locked', { cwd: root });
run('wasm-pack build', 'wasm-pack build --target web --no-opt -- --locked', { cwd: wasmDir });
