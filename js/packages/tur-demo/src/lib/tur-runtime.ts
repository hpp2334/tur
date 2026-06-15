let turApp: Record<string, unknown> | null = null;
let wasmReady: Promise<Record<string, unknown>> | null = null;

function fetchArrayBuffer(url: string): Promise<ArrayBuffer> {
    return new Promise((resolve, reject) => {
        const xhr = new XMLHttpRequest();
        xhr.open("GET", url, true);
        xhr.responseType = "arraybuffer";
        xhr.onload = () => {
            if (xhr.status >= 200 && xhr.status < 300) {
                resolve(xhr.response);
            } else {
                reject(new Error(`XHR ${xhr.status}: ${xhr.statusText}`));
            }
        };
        xhr.onerror = () => reject(new Error("XHR error"));
        xhr.send();
    });
}

async function loadAndInitWasm(): Promise<Record<string, unknown>> {
    if (wasmReady) return wasmReady;
    wasmReady = (async () => {
        const mod = await import(
            /* webpackIgnore: true */
            "./tur_wasm.js"
        );
        const buffer = await fetchArrayBuffer("tur_wasm_bg.wasm");
        const compiled = await WebAssembly.compile(buffer);
        await mod.default(compiled);
        return mod as Record<string, unknown>;
    })();
    return wasmReady;
}

export async function initTur(containerId: string): Promise<void> {
    try {
        const { TurWasmApp } = (await loadAndInitWasm()) as {
            TurWasmApp: {
                create_in: (id: string) => Promise<Record<string, unknown>>;
            };
        };
        turApp = await TurWasmApp.create_in(containerId);
        (globalThis as Record<string, unknown>).turApp = turApp;
        (globalThis as Record<string, unknown>).turDemo = {
            debugLayout: () => debugLayout(),
        };
    } catch (e) {
        wasmReady = null;
        const msg = e instanceof Error ? e.message : String(e);
        console.error("Tur init error:", msg);
        throw e;
    }
}

export function debugLayout(): string {
    try {
        const app = turApp as { debug_layout?: () => string } | null;
        return app?.debug_layout?.() ?? "";
    } catch {
        return "";
    }
}

let runSourceChain: Promise<void> = Promise.resolve();

export async function runSource(jsSource: string): Promise<void> {
    // Serialize runs — concurrent calls would race on destroyAndRecreate
    // (the wasm app gets torn down while another call is mid-load).
    const prev = runSourceChain;
    let resolve!: () => void;
    runSourceChain = new Promise<void>((r) => {
        resolve = r;
    });
    try {
        await prev.catch(() => {});
        await destroyAndRecreate();
        (turApp as { load_and_run_js: (s: string) => void }).load_and_run_js(
            jsSource,
        );
    } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        console.error("Tur runSource error:", msg);
        throw e;
    } finally {
        resolve();
    }
}

async function destroyAndRecreate(): Promise<void> {
    if (!turApp) return;
    const container = document.getElementById("tur-container");
    if (!container) {
        console.error("Tur destroyAndRecreate: container not found");
        return;
    }
    container.innerHTML = "";
    const { TurWasmApp } = (await loadAndInitWasm()) as {
        TurWasmApp: {
            create_in: (id: string) => Promise<Record<string, unknown>>;
        };
    };
    turApp = await TurWasmApp.create_in("tur-container");
    (globalThis as Record<string, unknown>).turApp = turApp;
}
