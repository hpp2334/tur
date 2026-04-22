import fs from "fs/promises";
import path from "path";
import { buildWasm, pkgDir } from "../build";
import { generateHtml } from "../html";
import { Server } from "../server";
import { Watcher } from "../watcher";

interface ServeOptions {
  port: string;
  build: boolean;
}

async function copyWasmFiles(serveDir: string): Promise<void> {
  const wasmFiles = await fs.readdir(pkgDir);
  for (const file of wasmFiles) {
    await fs.copyFile(path.join(pkgDir, file), path.join(serveDir, file));
  }
}

export async function serveAction(jsFile: string, options: ServeOptions): Promise<void> {
  const jsPath = path.resolve(jsFile);
  try {
    await fs.access(jsPath);
  } catch {
    console.error(`File not found: ${jsPath}`);
    process.exit(1);
  }

  if (options.build) {
    buildWasm();
  }

  const serveDir = path.dirname(jsPath);
  const jsFilename = path.basename(jsPath);
  await copyWasmFiles(serveDir);
  await fs.writeFile(path.join(serveDir, "index.html"), generateHtml(jsFilename));

  console.log(`Copied wasm files to ${serveDir}`);
  const server = new Server(serveDir, parseInt(options.port, 10), "0.0.0.0");
  server.start();

  async function reload() {
    await copyWasmFiles(serveDir);
    await fs.writeFile(path.join(serveDir, "index.html"), generateHtml(jsFilename));
    server.broadcast("reload");
  }

  const watcher = new Watcher();
  watcher.watch([jsPath, pkgDir], reload);

  async function shutdown() {
    console.log("\nShutting down...");
    watcher.close();
    await server.close();
    process.exit(0);
  }

  process.on("SIGTERM", shutdown);
  process.on("SIGINT", shutdown);
}
