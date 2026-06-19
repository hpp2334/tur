/** Shared types across the playground's state layer. */

export interface EditorController {
    setSpans(spans: Array<{ content: string; color?: unknown }>): void;
    setSpansPreserveCursor(
        spans: Array<{ content: string; color?: unknown }>,
    ): void;
    readonly text: string;
}

export type LayoutMode = "split" | "editor" | "viewer";
