/** Shared types across the playground's state layer. */

import type { TextController } from "builtin:tur/core";

/** The editor's text controller — the real `TextController` surface from
 *  `builtin:tur/core`. */
export type EditorController = TextController;

export type LayoutMode = "split" | "editor" | "viewer";

/** A per-case file cache: filename → current editor text. */
export type CaseFileMap = Record<string, string>;
