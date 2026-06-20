import type { LayoutMode } from "./types";

/** Format `ms - nowMs` as "just now" / "Xs ago" / "Xm ago" / "Xh ago". */
export function relativeTime(ms: number, nowMs: number): string {
    const diff = Math.max(0, nowMs - ms);
    const s = Math.floor(diff / 1000);
    if (s < 5) return "just now";
    if (s < 60) return `${s}s ago`;
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    return `${h}h ago`;
}

/** Map a layout mode to the `Expanded.flex` value for each pane.
 *  `split` → 1:1, `editor` → editor visible / viewer hidden,
 *  `viewer` → viewer visible / editor hidden. With the engine's flex
 *  algorithm honoring `Expanded.flex`, a `flex: 0` child collapses to
 *  zero width (its `min=max=0` constraint). */
export function layoutFlex(who: "editor" | "viewer", mode: LayoutMode): number {
    if (mode === "editor") return who === "editor" ? 1 : 0;
    if (mode === "viewer") return who === "editor" ? 0 : 1;
    return 1; // split
}
