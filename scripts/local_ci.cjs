const { execSync, spawnSync, spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

const rootDir = path.join(__dirname, '..');
const actrcPath = path.join(rootDir, '.actrc');
const platform = os.platform();

const sshPath = platform === 'darwin' ? '/Users/a/.ssh' : '/home/a/.ssh';

// Persistent CI caches live under the user's home — NOT /tmp, which is
// tmpfs here (RAM-backed, wiped on reboot): a warm cache would vanish and
// eat memory while it lives.
const cacheRoot = path.join(os.homedir(), '.tur-build-cache');

// ── Docker access ─────────────────────────────────────────────────────────
// act drives the Docker daemon, and the socket is root:docker 0660 — so the
// invoking shell needs the `docker` group. A shell started before the user
// was added to it (or any non-login shell, e.g. the agent harness) carries a
// stale group set and gets "permission denied" from the API, which act
// surfaces as an immediate, confusing failure. Rather than demanding a
// re-login, re-exec this script with the group applied through sudo
// (`sudo -u <user> -g docker`) — that keeps the run's own artifacts (logs/,
// .actrc) owned by the invoking user instead of root.
function dockerReachable() {
  try {
    execSync('docker info', { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

if (!dockerReachable()) {
  if (process.env.TUR_CI_REEXEC === '1') {
    console.error(
      'docker is still unreachable with the `docker` group applied — is the daemon running?',
    );
    process.exit(1);
  }
  const reexec = spawnSync(
    'sudo',
    [
      '-n',
      '-u',
      os.userInfo().username,
      '-g',
      'docker',
      'env',
      `PATH=${process.env.PATH}`,
      `HOME=${process.env.HOME}`,
      'TUR_CI_REEXEC=1',
      process.execPath,
      __filename,
      ...process.argv.slice(2),
    ],
    { cwd: rootDir, stdio: 'inherit' },
  );
  if (reexec.error) throw reexec.error;
  process.exit(reexec.status ?? 1);
}

const actrcContent = `-P ubuntu-latest=tur-ci:latest
--pull=false
--container-options --network=host -v ${cacheRoot}/cargo/registry:/root/.cargo/registry -v ${cacheRoot}/cargo/git:/root/.cargo/git -v ${cacheRoot}/sccache:/root/.cache/sccache -v ${cacheRoot}/pnpm/store:/root/.local/share/pnpm/store -v ${sshPath}:/root/.ssh:ro`;

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

// ── Stale act containers ──────────────────────────────────────────────────
// act names its job container deterministically (act-<workflow>-<job>-<hash>).
// A container left behind by an aborted or killed run collides with the next
// run's creation, so reap it up front; `--rm` below keeps successful runs from
// leaving one behind in the first place.
try {
  const stale = execSync("docker ps -aq --filter 'name=act-Local-CI-'", { encoding: 'utf8' })
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
  if (stale.length > 0) {
    console.log(`Reaping ${stale.length} stale act container(s)...`);
    execSync(`docker rm -f ${stale.join(' ')}`, { stdio: 'ignore' });
  }
} catch {
  // Listing is best-effort: never let cleanup block the run.
}

const currentBranch = execSync('git branch --show-current', { encoding: 'utf8' }).trim();

// Isolate this run's sccache daemon on its own port. The container shares
// the host netns (--network=host), so with the default port a run would
// either collide with or silently ADOPT a leaked daemon from an earlier
// killed run — whose cache dir is gone, silently defeating the persistent
// sccache mount (observed: a 0-byte cache across runs, every run compiling
// cold). A per-run port guarantees a fresh daemon owning the mounted cache;
// it dies with the container, and any future leak is inert.
const sccachePort = 20000 + Math.floor(Math.random() * 12000);
console.log(`sccache daemon port: ${sccachePort}`);

const logDir = path.join(rootDir, 'logs');
fs.mkdirSync(logDir, { recursive: true });
const logPath = path.join(logDir, 'workflow.log');
try {
  fs.rmSync(logPath, { force: true });
} catch {
  // A previous run driven as another user (e.g. root) left logs/ unwritable.
  execSync(`sudo -n chown -R ${process.getuid()}:${process.getgid()} ${logDir}`);
  fs.rmSync(logPath, { force: true });
}
const logStream = fs.createWriteStream(logPath, { flags: 'w' });

const child = spawn(
  'flock',
  [
    '/tmp/tur-ci.lock',
    'act',
    'workflow_dispatch',
    '--rm',
    '-W',
    '.github/workflows/local-ci.yml',
    '--json',
    '--env',
    `BRANCH_NAME=${currentBranch}`,
    '--env',
    `SCCACHE_SERVER_PORT=${sccachePort}`,
    // Linux: the ~/.ssh bind-mount lands at /root/.ssh still owned by the
    // host uid, so ssh rejects the routed config ("Bad owner or permissions").
    // Bypass the config with an explicit direct-connection command (github
    // :22 is reachable directly here; the image's keyscan proves it). On
    // macOS Docker Desktop presents the mount as root-owned, so the config
    // is usable as-is.
    ...(platform === 'darwin'
      ? []
      : [
          '--env',
          'GIT_SSH_COMMAND=ssh -F /dev/null -o IdentityFile=/root/.ssh/id_ed25519 -o UserKnownHostsFile=/root/.ssh/known_hosts -o StrictHostKeyChecking=yes',
        ]),
  ],
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
  process.exit(code ?? 1);
});
