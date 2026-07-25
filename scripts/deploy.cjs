const { execSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const root = path.join(__dirname, "..");
const jsDir = path.join(root, "js");
const distDir = path.join(root, "demo", "website", "dist");
const projectName = "tur-website";

function run(cmd, opts) {
  console.log(`> ${cmd}`);
  execSync(cmd, { stdio: "inherit", ...opts });
}

function getWranglerBin() {
  const bin = path.join(__dirname, "node_modules", ".bin", "wrangler");
  if (!fs.existsSync(bin)) {
    console.error("wrangler not found. Run `pnpm install` in scripts/ first.");
    process.exit(1);
  }
  return bin;
}

run("pnpm install", { cwd: jsDir });
run("pnpm build", { cwd: jsDir });

if (!fs.existsSync(distDir)) {
  console.error(`dist directory not found: ${distDir}`);
  process.exit(1);
}

const wrangler = getWranglerBin();
run(
  `${wrangler} pages deploy ${distDir} --project-name=${projectName} --commit-dirty=true --branch=main`,
  { cwd: root },
);
