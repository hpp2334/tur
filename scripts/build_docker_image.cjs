const { execSync } = require('child_process');
const path = require('path');

const rootDir = path.join(__dirname, '..');
const dockerfilePath = path.join(rootDir, 'docker', 'ci.Dockerfile');
const contextDir = path.join(rootDir, 'docker');
const imageName = 'tur-ci:latest';

execSync(
  `docker build --network=host -t ${imageName} -f ${dockerfilePath} ${contextDir}`,
  { stdio: 'inherit' },
);
