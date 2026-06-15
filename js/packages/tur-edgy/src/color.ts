declare const __tur: {
    createColor(r: number, g: number, b: number, a: number): unknown;
    createLinearGradient(
        sx: number,
        sy: number,
        ex: number,
        ey: number,
        stops: Array<{
            offset: number;
            r: number;
            g: number;
            b: number;
            a: number;
        }>,
    ): unknown;
};

export interface GradientStop {
    offset: number;
    color: Color;
}

export interface LinearGradientOptions {
    start: [number, number];
    end: [number, number];
    stops: GradientStop[];
}

/**
 * Color value backed by a Rust opaque (`ColorOpaque`).  The static factory
 * methods return the opaque JS object directly — NOT a wrapper — so that
 * the Rust bridge can read it via `downcast_ref::<ColorOpaque>()`.
 *
 * TS sees the return type as `Color` (via `as unknown as Color`) but at
 * runtime the value is the opaque handle.
 */
export class Color {
    private constructor() {}

    static rgb(r: number, g: number, b: number): Color {
        return __tur.createColor(r, g, b, 255) as unknown as Color;
    }

    static rgba(r: number, g: number, b: number, a: number): Color {
        return __tur.createColor(r, g, b, a) as unknown as Color;
    }

    static hex(hex: string): Color {
        const h = hex.replace(/^#/, "");
        let r: number, g: number, b: number, a: number;
        if (h.length === 3) {
            r = parseInt(h[0] + h[0], 16);
            g = parseInt(h[1] + h[1], 16);
            b = parseInt(h[2] + h[2], 16);
            a = 255;
        } else if (h.length === 6) {
            r = parseInt(h.slice(0, 2), 16);
            g = parseInt(h.slice(2, 4), 16);
            b = parseInt(h.slice(4, 6), 16);
            a = 255;
        } else if (h.length === 8) {
            r = parseInt(h.slice(0, 2), 16);
            g = parseInt(h.slice(2, 4), 16);
            b = parseInt(h.slice(4, 6), 16);
            a = parseInt(h.slice(6, 8), 16);
        } else {
            throw new Error(`Invalid hex color: ${hex}`);
        }
        return __tur.createColor(r, g, b, a) as unknown as Color;
    }
}

/**
 * LinearGradient value backed by a Rust opaque (`BrushOpaque`).
 * Same pattern as Color — returns the opaque directly.
 */
export class LinearGradient {
    private constructor() {}

    static create(options: LinearGradientOptions): LinearGradient {
        const stops = options.stops.map((s) => ({
            offset: s.offset,
            r: (s.color as unknown as { r: number }).r,
            g: (s.color as unknown as { g: number }).g,
            b: (s.color as unknown as { b: number }).b,
            a: (s.color as unknown as { a: number }).a,
        }));
        return __tur.createLinearGradient(
            options.start[0],
            options.start[1],
            options.end[0],
            options.end[1],
            stops,
        ) as unknown as LinearGradient;
    }
}
