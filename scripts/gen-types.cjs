const { execSync } = require('child_process');
const path = require('path');

execSync('cargo test -p tur-shared --lib export_bindings', {
  cwd: path.join(__dirname, '..'),
  stdio: 'inherit',
});
