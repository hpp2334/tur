import { atom } from "jotai";
import { initTur, runSource } from "../tur-runtime";

export function createViewerAtoms() {
    const turReady = atom(false);
    const turError = atom<string | null>(null);

    const initTurAction = atom(null, async (_get, set, containerId: string) => {
        try {
            await initTur(containerId);
            set(turReady, true);
        } catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            console.error("Tur init error:", msg);
            set(turError, msg);
        }
    });

    const runAction = atom(null, async (get, set, source: string) => {
        if (!get(turReady)) return;
        set(turError, null);
        try {
            await runSource(source);
        } catch (e) {
            const msg = e instanceof Error ? e.message : String(e);
            console.error("Tur runtime error:", msg);
            set(turError, msg);
        }
    });

    return { turReady, turError, initTur: initTurAction, run: runAction };
}

export const viewerAtoms = createViewerAtoms();
