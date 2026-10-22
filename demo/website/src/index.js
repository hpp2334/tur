// tur website — thin browser host. Loads the tur WASM (the website's own
// `tur-website` cdylib, which wraps the pure `tur-wasm` embedder lib + adds the
// demo-helper plugin), then loads the playground rut module
// (`rut/playground`, embedded in the cdylib) via `loadAndRunRutModule`. The
// entire playground UI (sidebar / editor / viewer) is rendered by tur itself.
//
// Plain ES module — no build step. `index.html` loads this file directly and
// the wasm glue (`tur_website.js`, staged by scripts/build.mjs) is imported
// natively at runtime.

let wasmReady = null;

function fetchArrayBuffer(url) {
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

// Detect the wasm multi-threaded backend's two hard browser
// requirements directly: `SharedArrayBuffer` must be constructible,
// and the document must be `crossOriginIsolated` (which requires
// COOP=`same-origin` + COEP=`require-corp` from the server). Any
// failure surfaces as a friendly red banner with diagnostics instead
// of an opaque wasm trap.
function browserDiagnostics() {
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

function probeBrowserSupport() {
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

async function loadWasm() {
    if (wasmReady) return wasmReady;
    wasmReady = (async () => {
        // The wasm glue (`tur_website.js`) is served from the site root (dev:
        // the pkg dir mounted by scripts/dev.mjs; prod: staged by
        // scripts/build.mjs). Imported natively — no bundler involved. The
        // specifiers are ABSOLUTE: this module lives at /src/, and a relative
        // `./` would resolve against /src/ (the old rspack bundle sat at the
        // site root, which is the resolution these paths preserve).
        const mod = await import("/tur_website.js");
        const buffer = await fetchArrayBuffer("/tur_website_bg.wasm");
        const compiled = await WebAssembly.compile(buffer);
        await mod.default(compiled);
        // No JS-side thread-pool init needed: the engine's in-tree
        // `worker_spawn` boots Web Workers on demand from Rust.
        return mod;
    })();
    return wasmReady;
}

function showFatalError(message, diag) {
    const overlay = document.getElementById("overlay");
    const status = document.getElementById("status");
    const msgEl = document.getElementById("error-message");
    const diagEl = document.getElementById("error-diag");
    if (overlay) overlay.classList.add("error");
    if (status) status.style.display = "none";
    if (msgEl) msgEl.textContent = message;
    if (diagEl) diagEl.textContent = diag;
}

async function main() {
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
        const { TurWebsiteApp } = mod;

        if (status) status.textContent = "booting tur…";
        const app = await TurWebsiteApp.create();
        globalThis.turApp = app;
        globalThis.turDevTool = app.dev_tool();

        if (status) status.textContent = "loading playground…";
        // The playground is a rut module embedded in the wasm host (the
        // Phase-4 rewrite): the site shell only feeds it to the engine's
        // zero-JS load path. The declared `menu` child (the editor's
        // context menu) rides the multi-file twin — its own scope carries
        // the menu's async surfaces.
        const source = app.playgroundSource();
        const menuSource = app.playgroundMenuSource();

        app.loadAndRunRutModuleFiles(source, { menu: menuSource });
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
