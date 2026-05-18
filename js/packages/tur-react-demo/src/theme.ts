import { Color } from "@tur/react-renderer";

export const Colors = {
  BG_APP: Color.hex("#f8fafc"),
  BG_SIDEBAR: Color.hex("#0f172a"),
  BG_SIDEBAR_ACTIVE: Color.hex("#1e293b"),
  BG_CARD: Color.hex("#ffffff"),

  PRIMARY: Color.hex("#6366f1"),
  PRIMARY_DARK: Color.hex("#4f46e5"),
  PRIMARY_LIGHT: Color.hex("#eef2ff"),

  TEXT_PRIMARY: Color.hex("#1e293b"),
  TEXT_SECONDARY: Color.hex("#64748b"),
  TEXT_MUTED: Color.hex("#94a3b8"),
  TEXT_WHITE: Color.hex("#ffffff"),

  SUCCESS: Color.hex("#22c55e"),
  SUCCESS_LIGHT: Color.hex("#f0fdf4"),
  DANGER: Color.hex("#ef4444"),
  DANGER_LIGHT: Color.hex("#fef2f2"),

  BORDER: Color.hex("#e2e8f0"),
  SHADOW: Color.rgba(0, 0, 0, 60),
  MODAL_BACKDROP: Color.rgba(0, 0, 0, 102),
} as const;
