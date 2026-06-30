// Design tokens — the single source of truth for visual decisions.
//
// This is the only file (besides cases/compile.ts, which owns the code-
// highlighting palette) where `Color.hex(...)` is allowed. Views import
// from `tokens` and never touch primitives directly. See DESIGN-SYSTEM.md §8.
//
// Light theme. Saturation: ink.* (warm-tinted neutrals) + teal.* (saturated
// cyan-leaning signature) + coral.* (warm complement) + status.* (AA-verified).

import { Color } from "@tur/edgy";

// ---------------------------------------------------------------------------
// Primitive palette — do not import from views.
// ---------------------------------------------------------------------------

export const ink = {
    50: Color.hex("#fbfcfd"),
    100: Color.hex("#f4f6f9"),
    150: Color.hex("#eceff4"),
    200: Color.hex("#e1e5ec"),
    300: Color.hex("#d4d9e0"),
    400: Color.hex("#b8c0cc"),
    500: Color.hex("#8a94a3"),
    600: Color.hex("#5e6878"),
    700: Color.hex("#3a4250"),
    800: Color.hex("#1f2530"),
    900: Color.hex("#0a0e14"),
} as const;

export const teal = {
    200: Color.hex("#7df5d0"),
    300: Color.hex("#00e8b8"),
    400: Color.hex("#00c69a"),
    500: Color.hex("#00a886"),
    600: Color.hex("#008a6e"),
    700: Color.hex("#006e58"),
    800: Color.hex("#005440"),
} as const;

export const coral = {
    300: Color.hex("#ffb3a3"),
    400: Color.hex("#ff8a72"),
    500: Color.hex("#e85d44"),
    700: Color.hex("#b03a1f"),
} as const;

export const status = {
    success: Color.hex("#00a06b"),
    warning: Color.hex("#c47700"),
    error: Color.hex("#d63a2f"),
    info: teal[700],
} as const;

// Code-highlight palette — used by cases/compile.ts to color TSX tokens. Tuned
// for AA on `code.bg` (ink.50). See DESIGN-SYSTEM.md §1.1.
export const code = {
    bg: ink[50],
    fg: ink[800],
    keyword: Color.hex("#006e58"),
    string: Color.hex("#3f7d3f"),
    number: Color.hex("#b35900"),
    comment: ink[500],
    operator: ink[600],
    literal: Color.hex("#92400e"),
    // AST-derived semantic categories (see tur-wasm `highlight_tsx`).
    decl: Color.hex("#1e6fb8"), // 7 — fn/view/const/import/call-callee name
    jsxTag: coral[700], // 8 — JSX element tag name
    jsxAttr: Color.hex("#7c3aed"), // 9 — JSX attribute name
    type: teal[800], // 10 — interface/type name
    property: Color.hex("#c2185b"), // 11 — object-literal key / member `.prop`
} as const;

// ---------------------------------------------------------------------------
// Semantic layer — what views actually use.
// ---------------------------------------------------------------------------

export const tokens = {
    bg: {
        app: ink[50],
        panel: ink[100],
        elevated: ink[150],
        hover: ink[200],
        strongHover: ink[300],
        weakHover: ink[200],
        selected: teal[200],
        selectedHover: teal[300],
        controlTray: ink[200],
        controlTrayHover: ink[300],
        controlSelected: ink[50],
        header: ink[150],
        code: ink[50],
        viewer: ink[50],
        danger: Color.hex("#fff0ee"),
        button: {
            primary: teal[400],
            primaryHover: teal[300],
            primaryPressed: teal[500],
            secondary: ink[200],
            ghost: ink[50],
        },
    },
    text: {
        primary: ink[800],
        body: ink[700],
        secondary: ink[600],
        tertiary: ink[500],
        disabled: ink[400],
        onAccent: ink[900],
        onDanger: Color.hex("#8a1a14"),
        placeholder: ink[400],
        code: code.fg,
        link: teal[800],
        inverse: ink[50],
    },
    border: {
        subtle: ink[300],
        strong: ink[400],
        focus: teal[600],
    },
    accent: {
        primary: teal[400],
        primaryText: teal[800],
        solid: teal[500],
        cursor: teal[500],
        complement: coral[400],
    },
    status: {
        success: status.success,
        warning: status.warning,
        error: status.error,
        info: status.info,
    },
    shadow: {
        // Soft elevation — alpha is 0–255 (a≈80 ≈ 31% opacity).
        sm: Color.rgba(15, 23, 42, 80),
        md: Color.rgba(15, 23, 42, 120),
        lg: Color.rgba(15, 23, 42, 160),
    },
} as const;
