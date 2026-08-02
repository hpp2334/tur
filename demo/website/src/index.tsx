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
        // NOTE: thread-pool init is currently disabled because the
        // engine traps with "memory access out of bounds" during JS
        // module evaluation under atomics-enabled codegen, even when
        // the wasm memory is correctly shared. See
        // `.cargo/config.toml` for the full status / debugging trail.
        // Uncomment the block below to attempt the threaded path:
        //
        // const initThreadPool = mod.initThreadPool as (
        //     n: number,
        // ) => Promise<unknown>;
        // if (typeof initThreadPool === "function") {
        //     await initThreadPool(navigator.hardwareConcurrency || 4);
        // }
        return mod;
    })();
    return wasmReady;
}

async function main(): Promise<void> {
    const status = document.getElementById("status");
    try {
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
