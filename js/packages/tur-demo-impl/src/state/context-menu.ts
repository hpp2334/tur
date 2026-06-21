import { mutate, set, source } from "@tur/edgy";
import { editorCtrl } from "./case-store";

// ---------------------------------------------------------------------------
// Context menu state for the code editor. Right-click on the editor
// (Cmd+click on macOS, right-click elsewhere) opens a menu with Cut / Copy /
// Paste / Select All. Click-outside closes it.
// ---------------------------------------------------------------------------

export interface ContextMenuPos {
    readonly x: number;
    readonly y: number;
}

export const contextMenuOpen$ = source<boolean>(false);
// Two separate source atoms for x/y rather than one object source. Object
// property access during the derive recompute phase was tickling a boa
// borrow-conflict bug; sticking to plain numbers avoids it.
export const contextMenuX$ = source<number>(0);
export const contextMenuY$ = source<number>(0);

/** The engine fires `onContextMenu` with `{ local: {x,y}, global: {x,y} }`
 *  — the global position is canvas-relative, which is what we need to anchor
 *  the menu. */
interface ContextMenuEvent {
    local: { x: number; y: number };
    global: { x: number; y: number };
}

/** Open the menu at canvas-relative coordinates (from the right-click event).
 *  Coerce to numbers immediately so we don't hold the event object across
 *  `set` calls. */
export const openContextMenu = mutate((_ctx, ev: ContextMenuEvent) => {
    const g = ev?.global ?? { x: 0, y: 0 };
    set(contextMenuX$, Number(g.x) || 0);
    set(contextMenuY$, Number(g.y) || 0);
    set(contextMenuOpen$, true);
});

/** Close the menu (click-outside, escape, or after an action runs). */
export const closeContextMenu = mutate(() => {
    set(contextMenuOpen$, false);
});

/** Cut: copy selection to clipboard then delete it. The actual clipboard
 *  write is performed by the engine when Cmd+X is pressed; for the menu
 *  action we simulate by calling `clipboardWriteText` directly. */
export const cutSelection = mutate(() => {
    const text = editorCtrl.selectedText;
    if (text.length > 0) {
        editorCtrl.deleteSelection();
        // Best-effort clipboard write via the host bridge.
        const host = (
            globalThis as unknown as {
                __turHost?: { clipboardWriteText?: (s: string) => void };
            }
        ).__turHost;
        host?.clipboardWriteText?.(text);
    }
    set(contextMenuOpen$, false);
});

export const copySelection = mutate(() => {
    const text = editorCtrl.selectedText;
    if (text.length > 0) {
        const host = (
            globalThis as unknown as {
                __turHost?: { clipboardWriteText?: (s: string) => void };
            }
        ).__turHost;
        host?.clipboardWriteText?.(text);
    }
    set(contextMenuOpen$, false);
});

export const pasteFromClipboard = mutate(() => {
    // Paste via the host's clipboard-read bridge. The engine handles Cmd+V
    // natively via the paste event on the hidden textarea, but the
    // context-menu action needs an explicit call. `clipboardReadText` is
    // callback-based (the host resolves it on the next frame from within
    // the boa context, where a &mut Context is available).
    const host = (
        globalThis as unknown as {
            __turHost?: {
                clipboardReadText?: (cb: (text: string) => void) => void;
            };
        }
    ).__turHost;
    host?.clipboardReadText?.((text) => {
        if (text.length > 0) editorCtrl.insertText(text);
    });
    set(contextMenuOpen$, false);
});

export const selectAll = mutate(() => {
    editorCtrl.setSelection(0, editorCtrl.text.length);
    set(contextMenuOpen$, false);
});
