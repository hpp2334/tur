const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const rootDir = path.join(__dirname, '..');
const actrcPath = path.join(rootDir, '.actrc');
const os = require('os').platform();

const sshPath = os === 'darwin' ? '/Users/a/.ssh' : '/home/a/.ssh';

const actrcContent = `-P ubuntu-latest=tur-ci:latest
--pull=false
--container-options --network=host -v /tmp/tur-ci-cache/cargo/registry:/root/.cargo/registry -v /tmp/tur-ci-cache/cargo/git:/root/.cargo/git -v /tmp/tur-ci-cache/pnpm/store:/root/.local/share/pnpm/store -v ${sshPath}:/root/.ssh:ro`;

fs.writeFileSync(actrcPath, actrcContent, 'utf8');

function waitForDocker(maxRetries = 30) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      const cid = execSync('docker create --rm tur-ci:latest true 2>/dev/null', { encoding: 'utf8' }).trim();
      execSync(`docker rm ${cid} > /dev/null 2>&1`, { stdio: 'pipe' });
      return;
    } catch {
      if (i === 0) process.stdout.write('Waiting for Docker daemon...');
      process.stdout.write('.');
      execSync('sleep 1', { stdio: 'pipe' });
    }
  }
  throw new Error('Docker daemon did not become ready in time');
}
waitForDocker();
console.log(' Docker ready.');

const currentBranch = execSync('git branch --show-current', { encoding: 'utf8' }).trim();

execSync(
  `mkdir -p logs/ && rm -f logs/workflow.log && flock /tmp/tur-ci.lock act workflow_dispatch -W .github/workflows/local-ci.yml --env BRANCH_NAME=${currentBranch} 2>&1 | tee logs/workflow.log`,
  {
    stdio: 'inherit',
    env: { ...process.env, PATH: `/usr/local/opt/util-linux/bin:${process.env.PATH}` },
  },
);
