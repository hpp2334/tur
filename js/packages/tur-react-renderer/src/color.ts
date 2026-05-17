export class Color {
  readonly r: number;
  readonly g: number;
  readonly b: number;
  readonly a: number;

  private constructor(r: number, g: number, b: number, a: number) {
    this.r = r;
    this.g = g;
    this.b = b;
    this.a = a;
  }

  static rgb(r: number, g: number, b: number): Color {
    return new Color(r, g, b, 255);
  }

  static rgba(r: number, g: number, b: number, a: number): Color {
    return new Color(r, g, b, a);
  }

  static hex(hex: string): Color {
    const h = hex.replace(/^#/, "");
    if (h.length === 3) {
      return new Color(
        parseInt(h[0] + h[0], 16),
        parseInt(h[1] + h[1], 16),
        parseInt(h[2] + h[2], 16),
        255,
      );
    }
    if (h.length === 6) {
      return new Color(
        parseInt(h.slice(0, 2), 16),
        parseInt(h.slice(2, 4), 16),
        parseInt(h.slice(4, 6), 16),
        255,
      );
    }
    if (h.length === 8) {
      return new Color(
        parseInt(h.slice(0, 2), 16),
        parseInt(h.slice(2, 4), 16),
        parseInt(h.slice(4, 6), 16),
        parseInt(h.slice(6, 8), 16),
      );
    }
    throw new Error(`Invalid hex color: ${hex}`);
  }
}
