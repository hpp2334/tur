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
