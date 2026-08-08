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

// Detect the wasm multi-threaded backend's two hard browser
// requirements directly: `SharedArrayBuffer` must be constructible,
// and the document must be `crossOriginIsolated` (which requires
// COOP=`same-origin` + COEP=`require-corp` from the server). Any
// failure surfaces as a friendly red banner with diagnostics instead
// of an opaque wasm trap.
function browserDiagnostics(): string {
    const sabSupported = typeof SharedArrayBuffer !== "undefined";
    // COOP/COEP aren't directly observable from JS; the only signal
    // exposed is `self.crossOriginIsolated`, which becomes true only
    // when both COOP (`same-origin`) and COEP (`require-corp`) are set
    // correctly. Report them as inferred from that flag.
    const coi =
        typeof crossOriginIsolated !== "undefined" && crossOriginIsolated;
    return (
        `SAB=${sabSupported ? "yes" : "no"} ` +
        `COOP/COEP=${coi ? "yes" : "no"} ` +
        `crossOriginIsolated=${coi ? "true" : "false"}`
    );
}

function probeBrowserSupport(): {
    ok: boolean;
    reason?: string;
} {
    const sabSupported = typeof SharedArrayBuffer !== "undefined";
    if (!sabSupported) {
        return {
            ok: false,
            reason: "SharedArrayBuffer is not available in this browser.",
        };
    }
    if (typeof crossOriginIsolated === "undefined" || !crossOriginIsolated) {
        return {
            ok: false,
            reason: "Document is not cross-origin isolated (COOP/COEP headers missing or not honored by this browser).",
        };
    }
    return { ok: true };
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

function showFatalError(message: string, diag: string): void {
    const overlay = document.getElementById("overlay");
    const status = document.getElementById("status");
    const msgEl = document.getElementById("error-message");
    const diagEl = document.getElementById("error-diag");
    if (overlay) overlay.classList.add("error");
    if (status) status.style.display = "none";
    if (msgEl) msgEl.textContent = message;
    if (diagEl) diagEl.textContent = diag;
}

async function main(): Promise<void> {
    const status = document.getElementById("status");
    try {
        if (status) status.textContent = "checking browser support…";
        const probe = probeBrowserSupport();
        if (!probe.ok) {
            showFatalError(
                probe.reason ?? "Browser support probe failed with no reason.",
                browserDiagnostics(),
            );
            return;
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
        // Fade the overlay out, then drop it so the canvas owns the
        // viewport. The paint is gone before tur's first frame lands.
        const overlay = document.getElementById("overlay");
        if (overlay) {
            overlay.classList.add("fade-out");
            overlay.addEventListener("transitionend", () => overlay.remove(), {
                once: true,
            });
        }
    } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        console.error("tur-demo bootstrap error:", msg);
        showFatalError(msg, browserDiagnostics());
    }
}

void main();
