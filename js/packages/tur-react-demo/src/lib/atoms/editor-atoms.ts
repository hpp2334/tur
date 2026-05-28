import { atom } from "jotai";
import { compileWithFiles, initCompiler } from "../compiler";
import { fetchAllFiles, getCaseFiles } from "../cases";

export function createEditorAtoms() {
    const selectedCase = atom<string | null>(null);
    const selectedFile = atom("index.tsx");
    const caseFiles = atom<Map<string, string>>(new Map());
    const source = atom("");
    const compilerReady = atom(false);
    const compiledSource = atom<string | null>(null);
    const buildError = atom<string | null>(null);
    const building = atom(false);

    const initCompilerAction = atom(null, async (_get, set) => {
        try {
            await initCompiler();
            set(compilerReady, true);
        } catch (e) {
            console.error(
                "Compiler init failed:",
                e instanceof Error ? e.message : String(e),
            );
        }
    });

    const selectCase = atom(null, async (get, set, name: string) => {
        set(selectedCase, name);
        set(selectedFile, "index.tsx");
        set(compiledSource, null);
        set(buildError, null);
        try {
            const allFiles = await fetchAllFiles(name);
            set(caseFiles, allFiles);
            const indexSource = allFiles.get("index.tsx") ?? "";
            set(source, indexSource);
            if (!get(compilerReady)) return;
            const result = compileWithFiles("index.tsx", indexSource, allFiles);
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

    const selectFile = atom(null, (get, set, fileName: string) => {
        const files = get(caseFiles);
        const content = files.get(fileName);
        if (content !== undefined) {
            set(selectedFile, fileName);
            set(source, content);
        }
    });

    const save = atom(null, (get, set, editedSource: string) => {
        if (!get(compilerReady)) return;
        set(building, true);
        set(buildError, null);

        const currentFile = get(selectedFile);
        const allFiles = new Map(get(caseFiles));
        allFiles.set(currentFile, editedSource);
        set(caseFiles, allFiles);

        const indexSource = allFiles.get("index.tsx") ?? editedSource;
        const result = compileWithFiles("index.tsx", indexSource, new Map(allFiles));
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
        selectedFile,
        caseFiles,
        source,
        compilerReady,
        compiledSource,
        buildError,
        building,
        initCompiler: initCompilerAction,
        selectCase,
        selectFile,
        save,
    };
}

export const editorAtoms = createEditorAtoms();
