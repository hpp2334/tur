const { execSync } = require("child_process");
const path = require("path");

const root = path.join(__dirname, "..");
const jsDir = path.join(root, "js");

function run(cmd, opts) {
  console.log(`> ${cmd}`);
  execSync(cmd, { stdio: "inherit", ...opts });
}

run("pnpm install", { cwd: jsDir });
run("pnpm --filter @tur-ng/rspack-plugin build", { cwd: jsDir });
run("pnpm --filter @tur-ng/react build", { cwd: jsDir });
