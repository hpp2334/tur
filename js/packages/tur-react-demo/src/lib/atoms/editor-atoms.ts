import { atom } from "jotai";
import { compile, initCompiler } from "../compiler";
import { fetchSource } from "../cases";

export function createEditorAtoms() {
    const selectedCase = atom<string | null>(null);
    const source = atom("");
    const compilerReady = atom(false);
    const compiledSource = atom<string | null>(null);
    const buildError = atom<string | null>(null);
    const building = atom(false);

    const initCompilerAction = atom(null, async (_get, set) => {
        try {
            await initCompiler();
            set(compilerReady, true);
            const src = _get(source);
            if (!src) return;
            const result = compile(src);
            if (result.error) {
                console.error("Compile error:", result.error);
                set(buildError, result.error);
            } else if (result.code) {
                set(compiledSource, result.code);
            }
        } catch (e) {
            console.error(
                "Compiler init failed:",
                e instanceof Error ? e.message : String(e),
            );
        }
    });

    const selectCase = atom(null, async (get, set, name: string) => {
        set(selectedCase, name);
        set(compiledSource, null);
        set(buildError, null);
        try {
            const src = await fetchSource(name);
            set(source, src);
            if (!get(compilerReady)) return;
            const result = compile(src);
            if (result.error) {
                console.error("Compile error:", result.error);
                set(buildError, result.error);
            } else if (result.code) {
                set(compiledSource, result.code);
            }
        } catch (e) {
            console.error(
                "Failed to load case:",
                e instanceof Error ? e.message : String(e),
            );
            set(source, "");
        }
    });

    const save = atom(null, (get, set, editedSource: string) => {
        if (!get(compilerReady)) return;
        set(building, true);
        set(buildError, null);
        const result = compile(editedSource);
        set(building, false);
        if (result.error) {
            console.error("Compile error:", result.error);
            set(buildError, result.error);
        } else if (result.code) {
            set(compiledSource, result.code);
        }
    });

    return {
        selectedCase,
        source,
        compilerReady,
        compiledSource,
        buildError,
        building,
        initCompiler: initCompilerAction,
        selectCase,
        save,
    };
}

export const editorAtoms = createEditorAtoms();
