#!/usr/bin/env node
// tur website dev server — a zero-dependency static server replacing the old
// rspack devServer:
//
// - serves a VIRTUAL ROOT (the website dir first, then native/pkg mounted at
//   the top level), so dev needs no staging at all: index.html, src/index.js
//   and the wasm glue are all read from where they live;
// - always sends the COOP/COEP/CORP headers the multi-threaded wasm backend
//   requires (SharedArrayBuffer + Web Workers → `crossOriginIsolated`; Firefox
//   needs COEP `require-corp`, never `credentialless`), plus `no-store` so a
//   browser refresh always picks up fresh assets;
// - rebuilds the wasm at boot and — debounced — whenever Rust/rut sources
//   change (libs/, rut/ — the cases are compile-time-embedded in the cdylib —
//   and native/src). JS/HTML edits need NO build: just refresh.
//
// Requires Node >= 20 (recursive fs.watch on Linux).
//
// Flags:
//   --tunnel      also spawn `cloudflared tunnel … run tur-local`
//                 (replaces the old `concurrently` runner)
//   --skip-wasm   reuse an existing native/pkg build at boot

import { spawn } from "node:child_process";
import { createReadStream, existsSync, statSync, watch } from "node:fs";
import { createServer } from "node:http";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const websiteDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const nativeDir = join(websiteDir, "native");
const pkgDir = join(nativeDir, "pkg");
const repoRoot = resolve(websiteDir, "..", "..");
const PORT = 8080;

// Served in order: the website's own files win over the pkg's.
const ROOTS = [websiteDir, pkgDir];

// The multi-threaded wasm backend hard-requires cross-origin isolation.
const RESPONSE_HEADERS = {
    "Cross-Origin-Opener-Policy": "same-origin",
    "Cross-Origin-Embedder-Policy": "require-corp",
    "Cross-Origin-Resource-Policy": "same-origin",
    "Cache-Control": "no-store",
};

const MIME = {
    ".html": "text/html; charset=utf-8",
    ".js": "text/javascript; charset=utf-8",
    ".mjs": "text/javascript; charset=utf-8",
    ".wasm": "application/wasm",
    ".json": "application/json; charset=utf-8",
    ".jsonc": "application/json; charset=utf-8",
    ".css": "text/css; charset=utf-8",
    ".svg": "image/svg+xml",
    ".png": "image/png",
};

function buildWasm(reason) {
    return new Promise((resolveBuild) => {
        console.log(
            `[dev] wasm-pack build --target web --no-opt (${reason}) …`,
        );
        const t0 = Date.now();
        // Same invocation as scripts/build.mjs (see the flag notes there).
        const child = spawn(
            "wasm-pack",
            ["build", "--target", "web", "--no-opt"],
            { cwd: nativeDir, stdio: "inherit" },
        );
        child.on("exit", (code) => {
            const secs = ((Date.now() - t0) / 1000).toFixed(1);
            if (code === 0) {
                console.log(`[dev] wasm built in ${secs}s`);
            } else {
                console.error(
                    `[dev] wasm build FAILED (${code}) — fix and save again`,
                );
            }
            resolveBuild(code === 0);
        });
    });
}

function resolveFile(urlPath) {
    let p;
    try {
        p = decodeURIComponent(new URL(urlPath, "http://x").pathname);
    } catch {
        return null;
    }
    if (p.endsWith("/")) p += "index.html";
    const rel = p.replace(/^\/+/, "");
    if (rel === "") return null;
    for (const root of ROOTS) {
        const abs = join(root, rel);
        // Traversal guard: the joined path must stay inside the root.
        if (abs !== root && !abs.startsWith(`${root}/`)) continue;
        if (existsSync(abs) && statSync(abs).isFile()) return abs;
    }
    return null;
}

function startServer() {
    const server = createServer((req, res) => {
        const file = resolveFile(req.url);
        if (!file) {
            console.log(`[dev] 404 ${req.url}`);
            res.writeHead(404, {
                "Content-Type": "text/plain",
                ...RESPONSE_HEADERS,
            });
            res.end("not found");
            return;
        }
        const type = MIME[extname(file)] ?? "application/octet-stream";
        res.writeHead(200, { "Content-Type": type, ...RESPONSE_HEADERS });
        if (req.method === "HEAD") {
            res.end();
            return;
        }
        createReadStream(file).pipe(res);
    });
    server.listen(PORT, "0.0.0.0", () => {
        console.log(`[dev] serving ${websiteDir}`);
        console.log(`[dev]   + ${pkgDir} (mounted at /)`);
        console.log(`[dev] http://localhost:${PORT}/`);
    });
    return server;
}

function startWatcher() {
    // Sources that end up inside the cdylib — a change to any of them needs a
    // wasm-pack rebuild. `.rut` matters: rut/cases are embedded at compile
    // time (pg_compile), not served as assets.
    const watched = [
        join(repoRoot, "libs"),
        join(repoRoot, "rut"),
        join(nativeDir, "src"),
        join(nativeDir, "Cargo.toml"),
        join(repoRoot, "Cargo.toml"),
    ];
    const RELEVANT = new Set([".rs", ".rut", ".toml", ".jsonc"]);
    const DEBOUNCE_MS = 500;

    let timer = null;
    let building = false;
    let queued = false;

    const schedule = (what) => {
        if (!what) return console.log("[dev] change detected (unknown path)");
        if (!RELEVANT.has(extname(what))) return;
        console.log(`[dev] change: ${what}`);
        clearTimeout(timer);
        timer = setTimeout(run, DEBOUNCE_MS);
    };
    const run = async () => {
        if (building) {
            queued = true;
            return;
        }
        building = true;
        await buildWasm("sources changed");
        building = false;
        if (queued) {
            queued = false;
            run();
        }
    };

    for (const path of watched) {
        if (!existsSync(path)) continue;
        watch(path, { recursive: true }, (_event, filename) =>
            schedule(filename),
        );
    }
    console.log("[dev] watching libs/ rut/ native/src for wasm rebuilds");
}

function startTunnel() {
    const child = spawn(
        "cloudflared",
        ["tunnel", "--config", "cloudflared.yml", "run", "tur-local"],
        { cwd: websiteDir, stdio: "inherit" },
    );
    child.on("exit", (code) =>
        console.log(`[dev] cloudflared exited (${code})`),
    );
    const forward = (signal) => {
        child.kill(signal);
        process.exit(0);
    };
    process.on("SIGINT", () => forward("SIGINT"));
    process.on("SIGTERM", () => forward("SIGTERM"));
}

const skipWasm = process.argv.includes("--skip-wasm");
if (skipWasm && !existsSync(join(pkgDir, "tur_website_bg.wasm"))) {
    console.error("[dev] --skip-wasm but native/pkg has no build yet");
    process.exit(1);
}

const boot = skipWasm ? Promise.resolve(true) : buildWasm("boot");
boot.then((ok) => {
    if (!ok) process.exit(1);
    startServer();
    startWatcher();
    if (process.argv.includes("--tunnel")) startTunnel();
});
