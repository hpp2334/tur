const { execSync, spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

const rootDir = path.join(__dirname, '..');
const actrcPath = path.join(rootDir, '.actrc');
const os = require('os').platform();

const sshPath = os === 'darwin' ? '/Users/a/.ssh' : '/home/a/.ssh';

const actrcContent = `-P ubuntu-latest=tur-ci:latest
--pull=false
--container-options --network=host -v /tmp/tur-ci-cache/cargo/registry:/root/.cargo/registry -v /tmp/tur-ci-cache/cargo/git:/root/.cargo/git -v /tmp/tur-ci-cache/cargo/target:/root/.cargo/target -v /tmp/tur-ci-cache/pnpm/store:/root/.local/share/pnpm/store -v ${sshPath}:/root/.ssh:ro`;

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

const logDir = path.join(rootDir, 'logs');
fs.mkdirSync(logDir, { recursive: true });
const logPath = path.join(logDir, 'workflow.log');
const logStream = fs.createWriteStream(logPath, { flags: 'w' });

const child = spawn(
  'flock',
  ['/tmp/tur-ci.lock', 'act', 'workflow_dispatch', '-W', '.github/workflows/local-ci.yml', '--json', '--env', `BRANCH_NAME=${currentBranch}`],
  {
    cwd: rootDir,
    env: { ...process.env, PATH: `/usr/local/opt/util-linux/bin:${process.env.PATH}` },
    stdio: ['ignore', 'pipe', 'pipe'],
  },
);

let buffer = '';

function handleLine(line) {
  logStream.write(line + '\n');
  let obj;
  try { obj = JSON.parse(line); } catch { return; }
  const msg = obj.msg || '';
  const isStepStart = msg.startsWith('⭐ Run Main');
  const isStepResult = (obj.stepResult === 'success' || obj.stepResult === 'failure') && obj.stage === 'Main';
  if (isStepStart || isStepResult) {
    process.stdout.write(msg + '\n');
  }
}

child.stdout.on('data', (chunk) => {
  buffer += chunk;
  const lines = buffer.split('\n');
  buffer = lines.pop();
  for (const line of lines) {
    if (line.trim()) handleLine(line);
  }
});

child.stderr.on('data', (chunk) => {
  process.stderr.write(chunk);
});

child.on('close', (code) => {
  if (buffer.trim()) handleLine(buffer);
  logStream.end();
  if (code === 0) {
    console.log('\nCI passed. Full log: logs/workflow.log');
  } else {
    console.log(`\nCI FAILED (exit ${code}). See logs/workflow.log for details.`);
  }
  process.exit(code || 0);
});
