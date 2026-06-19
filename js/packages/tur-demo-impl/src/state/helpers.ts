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

/** Map a layout mode to the `Expanded.flex` value for each pane. */
export function layoutFlex(who: "editor" | "viewer", mode: LayoutMode): number {
    if (mode === "editor") return who === "editor" ? 2 : 1;
    if (mode === "viewer") return who === "editor" ? 1 : 2;
    return 1;
}
