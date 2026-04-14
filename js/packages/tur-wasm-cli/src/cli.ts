import { Command } from "commander";
import fs from "fs/promises";
import path from "path";
import { buildWasm, pkgDir } from "./build";
import { generateHtml } from "./html";
import { Server } from "./server";
import { watchFile } from "./watcher";

interface ServeOptions {
  port: string;
  build: boolean;
}

const program = new Command();
program
  .name("tur-wasm-cli")
  .description("CLI for building and serving tur wasm demos")
  .version("0.1.0");

program
  .command("build")
  .description("Build the tur-wasm package with wasm-pack")
  .action(() => {
    buildWasm();
  });

program
  .command("serve")
  .description("Build wasm and serve a JS demo app")
  .argument("<jsFile>", "Path to the JS bundle file to serve")
  .option("-p, --port <number>", "Port to serve on", "3000")
  .option("--no-build", "Skip wasm build step")
  .action(async (jsFile: string, options: ServeOptions) => {
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
    const wasmFiles = await fs.readdir(pkgDir);
    for (const file of wasmFiles) {
      await fs.copyFile(path.join(pkgDir, file), path.join(serveDir, file));
    }
    await fs.writeFile(path.join(serveDir, "index.html"), generateHtml(jsFilename));

    console.log(`Copied wasm files to ${serveDir}`);
    const server = new Server(serveDir, parseInt(options.port, 10));
    server.start();
    const watcher = watchFile(jsPath, () => server.broadcast("reload"));

    async function shutdown() {
      console.log("\nShutting down...");
      watcher.close();
      await server.close();
      process.exit(0);
    }

    process.on("SIGTERM", shutdown);
    process.on("SIGINT", shutdown);
  });

program.parse();
