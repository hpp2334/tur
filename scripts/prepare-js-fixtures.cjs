const { execSync } = require('child_process');
const path = require('path');

const root = path.join(__dirname, '..');
const jsDir = path.join(root, 'js');

execSync('pnpm install --frozen-lockfile', { cwd: jsDir, stdio: 'inherit' });
execSync('cargo test -p tur-engine --lib export_bindings --locked', { cwd: root, stdio: 'inherit' });
execSync('pnpm build', { cwd: jsDir, stdio: 'inherit' });
