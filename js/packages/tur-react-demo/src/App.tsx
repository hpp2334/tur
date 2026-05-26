import { useCallback, useEffect, useRef, useState } from "react";
import { CaseSelector } from "./components/CaseSelector";
import { CodeEditor } from "./components/CodeEditor";
import { TurViewer } from "./components/TurViewer";
import { cases, fetchSource } from "./lib/cases";
import "./App.css";

async function fetchWorkspaceDeps(): Promise<{ name: string; code: string }[]> {
    const [turReact, turReactRenderer] = await Promise.all([
        fetch("/deps/tur-react.js").then((r) => r.text()),
        fetch("/deps/tur-react-renderer.js").then((r) => r.text()),
    ]);
    return [
        { name: "@tur/react", code: turReact },
        { name: "@tur/react-renderer", code: turReactRenderer },
    ];
}

export function App() {
    const [selectedCase, setSelectedCase] = useState<string | null>(null);
    const [source, setSource] = useState<string>("");
    const [compiledSource, setCompiledSource] = useState<string | null>(null);
    const [building, setBuilding] = useState(false);
    const [buildError, setBuildError] = useState<string | null>(null);
    const [loading, setLoading] = useState(false);
    const [workerReady, setWorkerReady] = useState(false);
    const workerRef = useRef<Worker | null>(null);

    useEffect(() => {
        const worker = new Worker(
            new URL("./rspack.worker.ts", import.meta.url),
            { type: "module" },
        );
        worker.onmessage = (event) => {
            if (event.data.type === "init-done") {
                setWorkerReady(true);
                return;
            }
            if (event.data.type === "result") {
                const { compiled, error } = event.data;
                setBuilding(false);
                if (error) {
                    setBuildError(error);
                } else {
                    setBuildError(null);
                    setCompiledSource(compiled);
                }
            }
        };
        workerRef.current = worker;

        fetchWorkspaceDeps()
            .then((deps) => {
                worker.postMessage({ type: "init", deps });
            })
            .catch((e) => {
                console.error("Failed to fetch workspace deps:", e);
            });

        return () => {
            worker.terminate();
            workerRef.current = null;
        };
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
            if (!workerReady || !workerRef.current) return;
            setBuilding(true);
            setBuildError(null);
            workerRef.current.postMessage({
                type: "build",
                source: editedSource,
                caseName: selectedCase ?? "untitled",
            });
        },
        [selectedCase, workerReady],
    );

    return (
        <div className="app">
            <CaseSelector selectedCase={selectedCase} onSelect={handleSelectCase} />
            <div className="main-area">
                <div className="editor-panel">
                    <div className="editor-header">
                        <span>
                            {selectedCase
                                ? `${selectedCase}/index.tsx`
                                : "select a case"}
                        </span>
                        {loading && (
                            <span className="building-indicator">loading...</span>
                        )}
                        {building && (
                            <span className="building-indicator">building...</span>
                        )}
                        {!workerReady && (
                            <span className="building-indicator">initializing compiler...</span>
                        )}
                        {buildError && (
                            <span className="build-error" title={buildError}>
                                build error
                            </span>
                        )}
                    </div>
                    {selectedCase ? (
                        <CodeEditor
                            source={source}
                            onSave={handleSave}
                        />
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
