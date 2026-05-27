import { useEffect, useRef } from "react";
import { useAtomValue, useSetAtom } from "jotai";
import { editorAtoms } from "../lib/atoms/editor-atoms";
import { viewerAtoms } from "../lib/atoms/viewer-atoms";

export function TurViewer() {
    const containerRef = useRef<HTMLDivElement>(null);
    const turReady = useAtomValue(viewerAtoms.turReady);
    const turError = useAtomValue(viewerAtoms.turError);
    const compiledSource = useAtomValue(editorAtoms.compiledSource);
    const initTur = useSetAtom(viewerAtoms.initTur);
    const run = useSetAtom(viewerAtoms.run);

    useEffect(() => {
        initTur("tur-container");
    }, [initTur]);

    useEffect(() => {
        if (!turReady || !compiledSource) return;
        run(compiledSource);
    }, [turReady, compiledSource, run]);

    return (
        <div className="tur-viewer">
            <div className="tur-viewer-header">
                <span>tur viewer</span>
                {!turReady && <span className="status">initializing...</span>}
                {turReady && <span className="status ready">ready</span>}
            </div>
            <div className="tur-canvas-wrapper">
                <div
                    id="tur-container"
                    ref={containerRef}
                    className="tur-canvas"
                />
                {turError && <div className="tur-error">{turError}</div>}
            </div>
        </div>
    );
}
