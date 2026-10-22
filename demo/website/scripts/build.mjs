#!/usr/bin/env node
// tur website build — stages the wasm + shell into dist/ (no bundler):
//
//   dist/
//     index.html                       the shell (its script tag is static)
//     src/index.js                     the bootstrap, copied as-is
//     _headers                         Cloudflare Pages header config
//     tur_website.js                   wasm-bindgen glue (top level)
//     tur_website_bg.wasm              the cdylib
//     snippets/**                      wasm-bindgen inline modules — the
//                                      relative path layout the glue expects
//                                      at runtime (the engine worker script
//                                      lives under snippets/<crate-hash>/…)
//
// Replaces the old rspack build + WasmBuildPlugin emitter.
//
// Flags:
//   --skip-wasm   reuse an existing native/pkg build (skips wasm-pack)

import { execSync } from "node:child_process";
import {
    cpSync,
    existsSync,
    mkdirSync,
    readdirSync,
    rmSync,
    statSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const websiteDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const nativeDir = join(websiteDir, "native");
const pkgDir = join(nativeDir, "pkg");
const distDir = join(websiteDir, "dist");

function buildWasm() {
    console.log("[build] wasm-pack build --target web --no-opt …");
    const t0 = Date.now();
    // Multi-threaded engine workers (Web Workers via the SharedArrayBuffer).
    // The `+atomics` rustflags in `.cargo/config.toml` apply to ALL wasm32
    // builds regardless of profile, so workers spawn fine with `--no-opt`.
    //
    // `--no-opt` skips wasm-opt post-processing (which interacts badly with
    // boa_engine's AST codegen — "null reference produced" traps) and keeps
    // the default `panic = "unwind"` linkage.
    execSync("wasm-pack build --target web --no-opt", {
        cwd: nativeDir,
        stdio: "inherit",
    });
    console.log(
        `[build] wasm built in ${((Date.now() - t0) / 1000).toFixed(1)}s`,
    );
}

function stageDist() {
    rmSync(distDir, { recursive: true, force: true });
    mkdirSync(join(distDir, "src"), { recursive: true });

    // The shell + bootstrap (no transpile, no bundle — served verbatim).
    cpSync(join(websiteDir, "index.html"), join(distDir, "index.html"));
    cpSync(
        join(websiteDir, "src", "index.js"),
        join(distDir, "src", "index.js"),
    );

    // Deployment header config (Cloudflare Pages / wrangler convention).
    const publicDir = join(websiteDir, "public");
    if (existsSync(publicDir)) {
        for (const entry of readdirSync(publicDir)) {
            cpSync(join(publicDir, entry), join(distDir, entry));
        }
    }

    // wasm-pack glue + binary at the site root (the paths index.js imports).
    for (const file of ["tur_website.js", "tur_website_bg.wasm"]) {
        cpSync(join(pkgDir, file), join(distDir, file));
    }
    // Inline wasm-bindgen modules, preserving the snippets/ layout.
    if (existsSync(join(pkgDir, "snippets"))) {
        cpSync(join(pkgDir, "snippets"), join(distDir, "snippets"), {
            recursive: true,
        });
    }
}

function printManifest(dir, relPrefix = "") {
    for (const entry of readdirSync(dir)) {
        const abs = join(dir, entry);
        const rel = relPrefix ? `${relPrefix}/${entry}` : entry;
        if (statSync(abs).isDirectory()) {
            printManifest(abs, rel);
        } else {
            console.log(`[build]   ${rel}`);
        }
    }
}

const skipWasm = process.argv.includes("--skip-wasm");
if (!skipWasm) {
    buildWasm();
} else if (!existsSync(join(pkgDir, "tur_website_bg.wasm"))) {
    console.error("[build] --skip-wasm but native/pkg has no build yet");
    process.exit(1);
}

stageDist();
console.log("[build] staged dist/:");
printManifest(distDir);
