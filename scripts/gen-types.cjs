const { execSync } = require('child_process');
const path = require('path');

execSync('cargo test -p tur-shared --lib export_bindings', {
  cwd: path.join(__dirname, '..'),
  stdio: 'inherit',
  env: {
    ...process.env,
    TS_RS_EXPORT_DIR: path.join(
      __dirname,
      '..',
      'js',
      'packages',
      'tur-solidjs',
      'src',
    ),
  },
});
