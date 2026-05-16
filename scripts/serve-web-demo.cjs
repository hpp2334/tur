const { execSync } = require("child_process");
const path = require("path");

const root = path.join(__dirname, "..");
const jsDir = path.join(root, "js");
const demoDir = path.join(jsDir, "packages/tur-preact-demo");
const cliBin = path.join(jsDir, "packages/tur-wasm-cli/dist/cli.cjs");

function run(cmd, opts) {
  console.log(`> ${cmd}`);
  execSync(cmd, { stdio: "inherit", ...opts });
}

run("pnpm --filter @tur/preact-demo build", { cwd: jsDir });

run(`node ${cliBin} serve ${path.join(demoDir, "dist/bundle.js")}`, {
  cwd: root,
});
