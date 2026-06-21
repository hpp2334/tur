/** Shared types across the playground's state layer. */

export interface EditorController {
    setSpans(spans: Array<{ content: string; color?: unknown }>): void;
    setSpansPreserveCursor(
        spans: Array<{ content: string; color?: unknown }>,
    ): void;
    readonly text: string;
    readonly cursorPosition: number;
    readonly selectionAnchor: number;
    readonly selectionEnd: number;
    readonly selectedText: string;
    insertText(text: string): void;
    deleteSelection(): void;
    setSelection(anchor: number, end: number): void;
}

export type LayoutMode = "split" | "editor" | "viewer";

/** A per-case file cache: filename → current editor text. */
export type CaseFileMap = Record<string, string>;
