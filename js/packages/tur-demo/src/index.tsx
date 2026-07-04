// tur-demo — thin browser wrapper. Loads the tur WASM (which registers the
// swc-backed `__turHost` compiler services on creation), then fetches the
// `tur-demo-impl` bundle and evaluates it via `loadAndRunModule`. The entire
// playground UI (sidebar / editor / viewer) lives in tur-demo-impl and is
// rendered by tur itself.

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
        // The wasm glue (`tur_wasm.js`) is copied verbatim into dist by the
        // WasmBuildPlugin; load it as a plain runtime asset, not a bundled
        // module.
        const mod = (await import(
            /* webpackIgnore: true */ "./tur_wasm.js"
        )) as Record<string, unknown>;
        const buffer = await fetchArrayBuffer("./tur_wasm_bg.wasm");
        const compiled = await WebAssembly.compile(buffer);
        await (mod.default as (b: WebAssembly.Module) => Promise<unknown>)(
            compiled,
        );
        return mod;
    })();
    return wasmReady;
}

async function main(): Promise<void> {
    const status = document.getElementById("status");
    try {
        if (status) status.textContent = "loading wasm…";
        const mod = await loadWasm();
        const { TurWasmApp } = mod as {
            TurWasmApp: { create: () => Promise<Record<string, unknown>> };
        };

        if (status) status.textContent = "booting tur…";
        const app = await TurWasmApp.create();
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
