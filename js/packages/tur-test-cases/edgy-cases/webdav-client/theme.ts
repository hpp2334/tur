import { Color } from "@tur/edgy";

/** Light palette (Notion / Linear-style), shared across the case's screens. */
export const COLORS = {
    pageBg: Color.hex("#f8fafc"),
    panel: Color.hex("#ffffff"),
    border: Color.hex("#e2e8f0"),
    text: Color.hex("#0f172a"),
    textMuted: Color.hex("#64748b"),
    accent: Color.hex("#4f46e5"),
    accentFg: Color.hex("#ffffff"),
    danger: Color.hex("#ef4444"),
    dangerFg: Color.hex("#ffffff"),
    success: Color.hex("#16a34a"),
    errorBg: Color.hex("#fef2f2"),
    rowHover: Color.hex("#f1f5f9"),
    rowSelected: Color.hex("#eef2ff"),
    inputBg: Color.hex("#f8fafc"),
    inputBorder: Color.hex("#cbd5e1"),
    subtleButton: Color.hex("#f1f5f9"),
    subtleButtonFg: Color.hex("#334155"),
    backdrop: Color.rgba(15, 23, 42, 110),
    shadowLg: Color.rgba(15, 23, 42, 24),
    badgeBg: Color.hex("#eef2ff"),
    badgeFg: Color.hex("#4338ca"),
} as const;
