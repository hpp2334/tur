// Public surface of the state layer. Components and entry point import from
// here; internal state files import each other directly (not via this barrel)
// to avoid barrel→case-store→barrel cycles.

export {
    editorCtrl,
    getCaseComponent,
    getCaseFileNames,
    loadCase,
    recompile,
    resetCase,
    selectFile,
} from "./case-store";
export { layoutFlex, relativeTime } from "./helpers";
export {
    autoRun$,
    CASE_NAMES,
    compileVersion$,
    edited$,
    editorFlex$,
    errorMsg$,
    hoveredCase$,
    hoveredFile$,
    INITIAL_CASE,
    lastCompiledAtMs$,
    layoutHovered$,
    layoutMode$,
    now$,
    resetHovered$,
    runHovered$,
    selectedCase$,
    selectedFile$,
    sidebarWidth$,
    status$,
    viewerFlex$,
} from "./sources";
export type { EditorController, LayoutMode } from "./types";
