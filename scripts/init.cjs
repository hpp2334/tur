const { execSync } = require('child_process');
const path = require('path');

const root = path.join(__dirname, '..');

function run(cmd, opts) {
  console.log(`> ${cmd}`);
  execSync(cmd, { stdio: 'inherit', ...opts });
}

run('cargo build --workspace', { cwd: root });

run('cargo test -p tur-shared --lib export_bindings', { cwd: root });

run('pnpm install', { cwd: path.join(root, 'js') });

run('pnpm --filter @tur/solidjs-renderer build', { cwd: path.join(root, 'js') });
run('pnpm --filter @tur/solidjs build', { cwd: path.join(root, 'js') });
run('pnpm --filter @tur/rspack-plugin build', { cwd: path.join(root, 'js') });
