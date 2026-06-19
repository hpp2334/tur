// Public surface of the state layer. Components and entry point import from
// here; internal state files import each other directly (not via this barrel)
// to avoid barrel→case-store→barrel cycles.

export {
    editorCtrl,
    getCaseComponent,
    loadCase,
    recompile,
    resetCase,
} from "./case-store";
export { layoutFlex, relativeTime } from "./helpers";
export {
    autoRun$,
    CASE_NAMES,
    edited$,
    errorMsg$,
    hoveredCase$,
    INITIAL_CASE,
    lastCompiledAtMs$,
    layoutHovered$,
    layoutMode$,
    now$,
    resetHovered$,
    runHovered$,
    selectedCase$,
    status$,
} from "./sources";
export type { EditorController, LayoutMode } from "./types";
