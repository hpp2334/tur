import type { Compiler, RspackPluginInstance } from "@rspack/core";
import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { readdir } from "node:fs/promises";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PLUGIN_NAME = "TurRspackPlugin";

export interface TurRspackPluginOptions {
  wasmDir?: string;
  jsEntry?: string;
  noBuild?: boolean;
}

const __dirname = fileURLToPath(import.meta.url);

function findWorkspaceRoot(startDir: string): string {
  let dir = startDir;
  for (let i = 0; i < 20; i++) {
    try {
      readFileSync(join(dir, "Cargo.toml"));
      return dir;
    } catch {
      const parent = resolve(dir, "..");
      if (parent === dir) break;
      dir = parent;
    }
  }
  return startDir;
}

export class TurRspackPlugin implements RspackPluginInstance {
  private options: Required<TurRspackPluginOptions>;

  constructor(options: TurRspackPluginOptions = {}) {
    const root = findWorkspaceRoot(
      typeof __dirname === "string" ? resolve(__dirname, "..", "..", "..", "..") : process.cwd(),
    );
    this.options = {
      wasmDir: options.wasmDir ?? join(root, "libs", "tur-wasm"),
      jsEntry: options.jsEntry ?? "__JS_FILE__",
      noBuild: options.noBuild ?? false,
    };
  }

  apply(compiler: Compiler): void {
    const { wasmDir, jsEntry, noBuild } = this.options;
    const pkgDir = join(wasmDir, "pkg");
    const templateHtml = join(
      typeof __dirname === "string" ? __dirname : resolve(import.meta.dirname!),
      "template.html",
    );

    const buildWasm = () => {
      if (noBuild) return;
      compiler.getInfrastructureLogger(PLUGIN_NAME).info("Building WASM with wasm-pack...");
      execSync("wasm-pack build --target web --no-opt", {
        cwd: wasmDir,
        stdio: "inherit",
      });
    };

    compiler.hooks.beforeRun.tapPromise(PLUGIN_NAME, async () => {
      buildWasm();
    });

    compiler.hooks.watchRun.tapPromise(PLUGIN_NAME, async () => {
      buildWasm();
    });

    compiler.hooks.emit.tapPromise(PLUGIN_NAME, async (compilation) => {
      const logger = compilation.getLogger(PLUGIN_NAME);

      const files = await readdir(pkgDir);
      const wasmAssets = files.filter(
        (f) => f.endsWith(".js") || f.endsWith(".wasm") || f.endsWith(".d.ts"),
      );

      for (const file of wasmAssets) {
        const src = join(pkgDir, file);
        const content = readFileSync(src);
        const source = new compiler.webpack.sources.RawSource(content);
        compilation.emitAsset(file, source);
        logger.info(`Copied WASM asset: ${file}`);
      }

      let html = readFileSync(templateHtml, "utf-8");
      html = html.replace("__JS_FILE__", jsEntry);

      compilation.emitAsset(
        "index.html",
        new compiler.webpack.sources.RawSource(html),
      );
      logger.info("Generated index.html");
    });
  }
}
