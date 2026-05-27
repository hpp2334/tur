import { useCallback, useEffect, useState } from "react";
import { CaseSelector } from "./components/CaseSelector";
import { CodeEditor } from "./components/CodeEditor";
import { TurViewer } from "./components/TurViewer";
import { cases, fetchSource } from "./lib/cases";
import { compile, initCompiler } from "./lib/compiler";
import "./App.css";

export function App() {
    const [selectedCase, setSelectedCase] = useState<string | null>(null);
    const [source, setSource] = useState<string>("");
    const [compiledSource, setCompiledSource] = useState<string | null>(null);
    const [building, setBuilding] = useState(false);
    const [buildError, setBuildError] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);
    const [compilerReady, setCompilerReady] = useState(false);

    useEffect(() => {
        initCompiler()
            .then(() => setCompilerReady(true))
            .catch((e) => console.error("Failed to init compiler:", e));
    }, []);

    const handleSelectCase = useCallback((name: string) => {
        setSelectedCase(name);
        setCompiledSource(null);
        setBuildError(null);
        setLoading(true);
        fetchSource(name)
            .then((s) => setSource(s))
            .catch(() => setSource(""))
            .finally(() => setLoading(false));
    }, []);

    const handleSave = useCallback(
        (editedSource: string) => {
            if (!compilerReady) return;
            setBuilding(true);
            setBuildError(null);
            const result = compile(editedSource);
            setBuilding(false);
            if (result.error) {
                setBuildError(result.error);
            } else if (result.code) {
                setBuildError(null);
                setCompiledSource(result.code);
            }
        },
        [compilerReady],
    );

    return (
        <div className="app">
            <CaseSelector
                selectedCase={selectedCase}
                onSelect={handleSelectCase}
            />
            <div className="main-area">
                <div className="editor-panel">
                    <div className="editor-header">
                        <span>
                            {selectedCase
                                ? `${selectedCase}/index.tsx`
                                : "select a case"}
                        </span>
                        {loading && (
                            <span className="building-indicator">
                                loading...
                            </span>
                        )}
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
                        <CodeEditor source={source} onSave={handleSave} />
                    ) : (
                        <div className="editor-placeholder">
                            select a test case from the sidebar
                        </div>
                    )}
                </div>
                <div className="viewer-panel">
                    <TurViewer
                        caseName={selectedCase}
                        compiledSource={compiledSource}
                    />
                </div>
            </div>
        </div>
    );
}
