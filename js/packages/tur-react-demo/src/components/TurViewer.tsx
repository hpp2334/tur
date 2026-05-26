import { useEffect, useRef, useState } from "react";
import { initTur, loadCase, runSource } from "../lib/tur-runtime";

interface TurViewerProps {
    caseName: string | null;
    compiledSource?: string | null;
}

export function TurViewer({ caseName, compiledSource }: TurViewerProps) {
    const containerRef = useRef<HTMLDivElement>(null);
    const [ready, setReady] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (!containerRef.current) return;
        let cancelled = false;
        initTur("tur-container")
            .then(() => {
                if (!cancelled) setReady(true);
            })
            .catch((e) => {
                if (!cancelled) setError(e instanceof Error ? e.message : String(e));
            });
        return () => {
            cancelled = true;
        };
    }, []);

    useEffect(() => {
        if (!ready || !caseName) return;
        setError(null);
        if (compiledSource) {
            runSource(compiledSource).catch((e) =>
                setError(e instanceof Error ? e.message : String(e)),
            );
        } else {
            loadCase(caseName).catch((e) =>
                setError(e instanceof Error ? e.message : String(e)),
            );
        }
    }, [ready, caseName, compiledSource]);

    return (
        <div className="tur-viewer">
            <div className="tur-viewer-header">
                <span>tur viewer</span>
                {!ready && <span className="status">initializing...</span>}
                {ready && <span className="status ready">ready</span>}
            </div>
            <div className="tur-canvas-wrapper">
                <div
                    id="tur-container"
                    ref={containerRef}
                    className="tur-canvas"
                />
                {error && <div className="tur-error">{error}</div>}
            </div>
        </div>
    );
}
