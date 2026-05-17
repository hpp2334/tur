const { execSync } = require("child_process");
const path = require("path");

const root = path.join(__dirname, "..");
const jsDir = path.join(root, "js");

function run(cmd, opts) {
  console.log(`> ${cmd}`);
  execSync(cmd, { stdio: "inherit", ...opts });
}

run("pnpm install", { cwd: jsDir });
run("cargo test -p tur-shared --lib export_bindings --locked", { cwd: root });
run("pnpm --filter @tur/rspack-plugin build", { cwd: jsDir });
run("pnpm --filter @tur/react-renderer build", { cwd: jsDir });
run("pnpm --filter @tur/react build", { cwd: jsDir });
