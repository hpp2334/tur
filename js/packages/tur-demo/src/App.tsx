import { useAtomValue, useSetAtom } from "jotai";
import { useEffect } from "react";
import { CaseSelector } from "./components/CaseSelector";
import { CodeEditor } from "./components/CodeEditor";
import { TurViewer } from "./components/TurViewer";
import { editorAtoms } from "./lib/atoms/editor-atoms";
import "./App.css";

export function App() {
    const selectedCase = useAtomValue(editorAtoms.selectedCase);
    const selectedFile = useAtomValue(editorAtoms.selectedFile);
    const caseNames = useAtomValue(editorAtoms.caseNames);
    const source = useAtomValue(editorAtoms.source);
    const building = useAtomValue(editorAtoms.building);
    const buildError = useAtomValue(editorAtoms.buildError);
    const compilerReady = useAtomValue(editorAtoms.compilerReady);
    const selectCase = useSetAtom(editorAtoms.selectCase);
    const selectFile = useSetAtom(editorAtoms.selectFile);
    const save = useSetAtom(editorAtoms.save);
    const initCompiler = useSetAtom(editorAtoms.initCompiler);

    // biome-ignore lint/correctness/useExhaustiveDependencies: init only, selectCase is stable
    useEffect(() => {
        initCompiler().then(() => {
            selectCase("todolist");
        });
    }, [initCompiler]);

    return (
        <div className="app">
            <CaseSelector
                caseNames={caseNames}
                selectedCase={selectedCase}
                selectedFile={selectedFile}
                onSelectCase={selectCase}
                onSelectFile={selectFile}
            />
            <div className="main-area">
                <div className="editor-panel">
                    <div className="editor-header">
                        <span>
                            {selectedCase
                                ? `${selectedCase}/${selectedFile}`
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
