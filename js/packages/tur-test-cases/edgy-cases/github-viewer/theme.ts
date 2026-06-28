import { Color } from "@tur/edgy";

/** Light palette (Notion / Linear-style), shared across the case's screens. */
export const COLORS = {
    // Surfaces
    pageBg: Color.hex("#f8fafc"), // slate-50
    panel: Color.hex("#ffffff"),
    panelSunken: Color.hex("#f1f5f9"), // slate-100 — header strips, toolbars
    cardHover: Color.hex("#f8fafc"),

    // Lines
    border: Color.hex("#e2e8f0"), // slate-200
    divider: Color.hex("#eef2f6"), // very subtle row separators
    borderStrong: Color.hex("#cbd5e1"), // slate-300 — inputs, dashed cards

    // Text
    text: Color.hex("#0f172a"), // slate-900
    textMuted: Color.hex("#64748b"), // slate-500
    textSubtle: Color.hex("#94a3b8"), // slate-400 — captions, subtitles

    // Accent (indigo)
    accent: Color.hex("#4f46e5"), // indigo-600
    accentHover: Color.hex("#4338ca"), // indigo-700
    accentFg: Color.hex("#ffffff"),
    accentSoft: Color.hex("#eef2ff"), // indigo-50 — selected row, badges

    // Status
    danger: Color.hex("#ef4444"), // red-500
    dangerHover: Color.hex("#dc2626"), // red-600
    dangerFg: Color.hex("#ffffff"),
    dangerSoft: Color.hex("#fef2f2"), // red-50
    success: Color.hex("#16a34a"), // green-600

    // Rows / interactive surfaces
    rowHover: Color.hex("#f1f5f9"), // slate-100
    rowSelected: Color.hex("#eef2ff"), // indigo-50
    rowActive: Color.hex("#e0e7ff"), // indigo-100 — pressed / stronger select

    // Inputs
    inputBg: Color.hex("#f8fafc"),
    inputBorder: Color.hex("#cbd5e1"),
    inputFocusRing: Color.hex("#c7d2fe"), // indigo-200

    // Buttons
    subtleButton: Color.hex("#f1f5f9"),
    subtleButtonFg: Color.hex("#334155"), // slate-700
    subtleButtonHover: Color.hex("#e2e8f0"),

    // Misc
    backdrop: Color.rgba(15, 23, 42, 110),
    badgeBg: Color.hex("#eef2ff"),
    badgeFg: Color.hex("#4338ca"),

    // Shadows — soft, layered.
    shadowSm: Color.rgba(15, 23, 42, 6), // 0 1px 2px — buttons, chips
    shadowMd: Color.rgba(15, 23, 42, 8), // 0 1px 3px — cards
    shadowLg: Color.rgba(15, 23, 42, 22), // 0 12px 32px — modals
} as const;
