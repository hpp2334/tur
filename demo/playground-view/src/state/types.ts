/** Shared types across the playground's state layer. */

import type { TextController } from "tur:std";

/** The editor's text controller — the real `TextController` surface from
 *  `tur:std`. */
export type EditorController = TextController;

export type LayoutMode = "split" | "editor" | "viewer";

/** Active pane on mobile (driven by the bottom tab bar). Desktop uses
 *  `LayoutMode` instead — the two are independent so crossing the mobile
 *  breakpoint doesn't clobber either preference. */
export type MobileTab = "cases" | "edit" | "view";

/** A per-case file cache: filename → current editor text. */
export type CaseFileMap = Record<string, string>;
