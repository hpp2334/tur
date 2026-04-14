import { execSync } from "child_process";
import path from "path";

const root = path.join(import.meta.dirname, "../../../../");
const wasmDir = path.join(root, "libs/tur-wasm");
export const pkgDir = path.join(wasmDir, "pkg");

export function buildWasm(): void {
  console.log("> wasm-pack build --target web");
  execSync("wasm-pack build --target web", { cwd: wasmDir, stdio: "inherit" });
}
