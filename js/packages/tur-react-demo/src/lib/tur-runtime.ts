let turApp: Record<string, unknown> | null = null;

export async function initTur(containerId: string): Promise<void> {
    const initWasm = (globalThis as Record<string, unknown>).initTurWasm as
        | ((id: string) => Promise<unknown>)
        | undefined;
    if (!initWasm) {
        const msg = "WASM not loaded";
        console.error("Tur init error:", msg);
        throw new Error(msg);
    }
    try {
        turApp = (await initWasm(containerId)) as Record<string, unknown>;
        (globalThis as Record<string, unknown>).turDemo = {
            debugLayout: () => debugLayout(),
        };
    } catch (e) {
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

export async function runSource(jsSource: string): Promise<void> {
    try {
        await destroyAndRecreate();
        (turApp as { load_and_run_js: (s: string) => void }).load_and_run_js(
            jsSource,
        );
    } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        console.error("Tur runSource error:", msg);
        throw e;
    }
}

async function destroyAndRecreate(): Promise<void> {
    if (!turApp) return;
    const container = document.getElementById("tur-container");
    const initWasm = (globalThis as Record<string, unknown>).initTurWasm as
        | ((id: string) => Promise<unknown>)
        | undefined;
    if (!initWasm || !container) {
        console.error(
            "Tur destroyAndRecreate: WASM loader or container not found",
        );
        return;
    }
    container.innerHTML = "";
    turApp = (await initWasm("tur-container")) as Record<string, unknown>;
}
