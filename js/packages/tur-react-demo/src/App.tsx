import { useEffect } from "react";
import { useAtomValue, useSetAtom } from "jotai";
import { CaseSelector } from "./components/CaseSelector";
import { CodeEditor } from "./components/CodeEditor";
import { TurViewer } from "./components/TurViewer";
import { editorAtoms } from "./lib/atoms/editor-atoms";
import "./App.css";

export function App() {
    const selectedCase = useAtomValue(editorAtoms.selectedCase);
    const source = useAtomValue(editorAtoms.source);
    const building = useAtomValue(editorAtoms.building);
    const buildError = useAtomValue(editorAtoms.buildError);
    const compilerReady = useAtomValue(editorAtoms.compilerReady);
    const selectCase = useSetAtom(editorAtoms.selectCase);
    const save = useSetAtom(editorAtoms.save);
    const initCompiler = useSetAtom(editorAtoms.initCompiler);

    useEffect(() => {
        initCompiler();
    }, [initCompiler]);

    return (
        <div className="app">
            <CaseSelector
                selectedCase={selectedCase}
                onSelect={selectCase}
            />
            <div className="main-area">
                <div className="editor-panel">
                    <div className="editor-header">
                        <span>
                            {selectedCase
                                ? `${selectedCase}/index.tsx`
                                : "select a case"}
                        </span>
                        {building && (
                            <span className="building-indicator">
                                building...
                            </span>
                        )}
                        {!compilerReady && (
                            <span className="building-indicator">
                                initializing compiler...
                            </span>
                        )}
                        {buildError && (
                            <span className="build-error" title={buildError}>
                                build error
                            </span>
                        )}
                    </div>
                    {selectedCase ? (
                        <CodeEditor source={source} onSave={save} />
                    ) : (
                        <div className="editor-placeholder">
                            select a test case from the sidebar
                        </div>
                    )}
                </div>
                <div className="viewer-panel">
                    <TurViewer />
                </div>
            </div>
        </div>
    );
}
