let turApp: Record<string, unknown> | null = null;

export async function initTur(containerId: string): Promise<void> {
    turApp = await createApp(containerId);
}

export async function loadCase(caseName: string): Promise<void> {
    await destroyAndRecreate();
    const resp = await fetch(`/cases/${caseName}.js`);
    const source = await resp.text();
    const fn = turApp?.load_and_run_js as (s: string) => void;
    fn(source);
}

export async function runSource(jsSource: string): Promise<void> {
    await destroyAndRecreate();
    const fn = turApp?.load_and_run_js as (s: string) => void;
    fn(jsSource);
}

async function createApp(
    containerId: string,
): Promise<Record<string, unknown>> {
    const initWasm = (globalThis as Record<string, unknown>).initTurWasm as
        | ((id: string) => Promise<unknown>)
        | undefined;
    if (!initWasm) throw new Error("WASM not loaded");
    return (await initWasm(containerId)) as Record<string, unknown>;
}

async function destroyAndRecreate(): Promise<void> {
    if (!turApp) return;
    const container = document.getElementById("tur-container");
    const initWasm = (globalThis as Record<string, unknown>).initTurWasm as
        | ((id: string) => Promise<unknown>)
        | undefined;
    if (!initWasm || !container) return;
    container.innerHTML = "";
    turApp = (await initWasm("tur-container")) as Record<string, unknown>;
}
