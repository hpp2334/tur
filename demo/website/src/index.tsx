// tur website — thin browser host. Loads the tur WASM (the website's own
// `tur-website` cdylib, which wraps the pure `tur-wasm` embedder lib + adds the
// demo-helper plugin), then fetches the playground-view bundle and evaluates it
// via `loadAndRunModule`. The entire playground UI (sidebar / editor / viewer)
// lives in playground-view and is rendered by tur itself.

let wasmReady: Promise<Record<string, unknown>> | null = null;

function fetchArrayBuffer(url: string): Promise<ArrayBuffer> {
    return new Promise((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        xhr.open("GET", url, true);
        xhr.responseType = "arraybuffer";
        xhr.onload = () => {
            if (xhr.status >= 200 && xhr.status < 300) {
                resolve(xhr.response as ArrayBuffer);
            } else {
                reject(new Error(`XHR ${xhr.status}: ${xhr.statusText}`));
            }
        };
        xhr.onerror = () => reject(new Error("XHR error"));
        xhr.send();
    });
}

// End-to-end probe of the wasm multi-threaded backend's browser
// requirements. Spawn a Worker, hand it a `SharedArrayBuffer`, have it
// read the value main wrote + write its own value back, then verify
// both directions round-trip. This single probe covers:
//   - `SharedArrayBuffer` is constructible (requires COOP/COEP)
//   - `Worker` is spawnable from a blob URL
//   - `postMessage` can carry a `SharedArrayBuffer` (requires
//     `crossOriginIsolated`)
//   - The Worker can read + write the shared memory
// Any failure (browser doesn't support COOP/COEP, SAB missing, Worker
// can't see shared memory, etc.) surfaces as a friendly red banner
// instead of an opaque wasm trap.
async function probeBrowserSupport(): Promise<{
    ok: boolean;
    reason?: string;
}> {
    let url: string | null = null;
    let worker: Worker | null = null;
    try {
        const sab = new SharedArrayBuffer(4);
        const view = new Int32Array(sab);
        view[0] = 42;

        const src = `
self.onmessage = (e) => {
    const view = new Int32Array(e.data);
    const received = view[0];
    view[0] = received + 1;
    self.postMessage(received);
};`;
        url = URL.createObjectURL(
            new Blob([src], { type: "application/javascript" }),
        );
        worker = new Worker(url);

        const received = await new Promise<number>((resolve, reject) => {
            worker.onmessage = (e: MessageEvent) => resolve(e.data as number);
            worker.onerror = () =>
                reject(new Error("Worker failed to spawn or errored."));
            worker.postMessage(sab);
            setTimeout(
                () => reject(new Error("Worker probe timed out.")),
                2000,
            );
        });

        if (received !== 42) {
            return {
                ok: false,
                reason: `Worker read ${received}, expected 42.`,
            };
        }
        if (view[0] !== 43) {
            return {
                ok: false,
                reason: `Shared memory not shared: worker wrote but main read ${view[0]}.`,
            };
        }
        return { ok: true };
    } catch (err) {
        return {
            ok: false,
            reason:
                err instanceof Error
                    ? err.message
                    : `Probe threw: ${String(err)}`,
        };
    } finally {
        if (worker) worker.terminate();
        if (url) URL.revokeObjectURL(url);
    }
}

async function loadWasm(): Promise<Record<string, unknown>> {
    if (wasmReady) return wasmReady;
    wasmReady = (async () => {
        // The wasm glue (`tur_website.js`) is copied verbatim into dist by the
        // WasmBuildPlugin; load it as a plain runtime asset, not a bundled
        // module.
        const mod = (await import(
            /* webpackIgnore: true */ "./tur_website.js"
        )) as Record<string, unknown>;
        const buffer = await fetchArrayBuffer("./tur_website_bg.wasm");
        const compiled = await WebAssembly.compile(buffer);
        await (mod.default as (b: WebAssembly.Module) => Promise<unknown>)(
            compiled,
        );
        // No JS-side thread-pool init needed: `tur-engine`'s `ThreadedBackend`
        // uses `wasm_thread` (Web Workers spawn on demand from Rust via
        // `wasm_thread::spawn`). Compare to the previous wasm-bindgen-rayon
        // setup which required `await initThreadPool(n)` here.
        return mod;
    })();
    return wasmReady;
}

async function main(): Promise<void> {
    const status = document.getElementById("status");
    try {
        if (status) status.textContent = "checking browser support…";
        const probe = await probeBrowserSupport();
        if (!probe.ok) {
            throw new Error(
                `This browser cannot run tur's multi-threaded wasm backend: ${probe.reason} Try the latest Chrome or desktop Firefox/Chrome.`,
            );
        }

        if (status) status.textContent = "loading wasm…";
        const mod = await loadWasm();
        const { TurWebsiteApp } = mod as {
            TurWebsiteApp: { create: () => Promise<Record<string, unknown>> };
        };

        if (status) status.textContent = "booting tur…";
        const app = await TurWebsiteApp.create();
        (globalThis as Record<string, unknown>).turApp = app;
        (globalThis as Record<string, unknown>).turDevTool = (
            app as { dev_tool: () => unknown }
        ).dev_tool();

        if (status) status.textContent = "loading playground…";
        const resp = await fetch("./impl.js");
        if (!resp.ok)
            throw new Error(`failed to fetch impl.js: ${resp.status}`);
        const bundle = await resp.text();

        (app as { loadAndRunModule: (s: string) => void }).loadAndRunModule(
            bundle,
        );
        if (status) status.remove();
    } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        console.error("tur-demo bootstrap error:", msg);
        if (status) {
            status.textContent = `error: ${msg}`;
            status.style.color = "#ef4444";
        }
    }
}

void main();
