const { execSync } = require('child_process');
const path = require('path');

const rootDir = path.join(__dirname, '..');
const dockerfilePath = path.join(rootDir, 'docker', 'ci.Dockerfile');
const contextDir = rootDir;
const imageName = 'tur-ci:latest';

execSync(
  `docker build --ssh default --network=host -t ${imageName} -f ${dockerfilePath} ${contextDir}`,
  { stdio: 'inherit', env: { ...process.env, DOCKER_BUILDKIT: '1' } },
);
