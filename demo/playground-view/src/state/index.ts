// Public surface of the state layer. Views and entry point import from
// here; internal state files import each other directly (not via this barrel)
// to avoid barrel→case-store→barrel cycles.

export {
    editorCtrl,
    editorUndo,
    getCaseFileNames,
    loadCase,
    recompile,
    resetCase,
    runCase,
    selectFile,
} from "./case-store";
export {
    closeContextMenu,
    contextMenuOpen$,
    contextMenuX$,
    contextMenuY$,
    copySelection,
    cutSelection,
    openContextMenu,
    pasteFromClipboard,
    selectAll,
} from "./context-menu";
export { relativeTime } from "./helpers";
export {
    app$,
    autoRun$,
    CASE_NAMES,
    compileVersion$,
    edited$,
    editorWidth$,
    errorMsg$,
    hoveredCase$,
    hoveredFile$,
    INITIAL_CASE,
    isMobile$,
    lastCompiledAtMs$,
    layoutHovered$,
    layoutMode$,
    mobileTab$,
    now$,
    resetHovered$,
    runHovered$,
    selectedCase$,
    selectedFile$,
    sidebarWidth$,
    startNowTicker,
    status$,
    stopNowTicker,
} from "./sources";
export type { EditorController, LayoutMode, MobileTab } from "./types";
