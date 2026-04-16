import type { Color } from "@tur/solidjs-renderer";
import type { CrossAxisAlignment } from "@tur/solidjs-renderer";
import type { MainAxisAlignment } from "@tur/solidjs-renderer";
import type { ResolvedStyle } from "@tur/solidjs-renderer";

export const Flex = {
  gap(value: number): Partial<Pick<ResolvedStyle, "gap">> {
    return { gap: value };
  },
  mainAlignment(v: MainAxisAlignment): Partial<Pick<ResolvedStyle, "mainAlignment">> {
    return { mainAlignment: v };
  },
  crossAlignment(v: CrossAxisAlignment): Partial<Pick<ResolvedStyle, "crossAlignment">> {
    return { crossAlignment: v };
  },
};

export const TextOpts = {
  fontSize(value: number): Partial<Pick<ResolvedStyle, "fontSize">> {
    return { fontSize: value };
  },
};

export class Style {
  private data: ResolvedStyle = {
    mainAlignment: null,
    crossAlignment: null,
    gap: null,
    fontSize: null,
    color: null,
    padding: null,
    width: null,
    height: null,
  };

  flex(opts: Partial<Pick<ResolvedStyle, "mainAlignment" | "crossAlignment" | "gap">>): this {
    Object.assign(this.data, opts);
    return this;
  }

  text(opts: Partial<Pick<ResolvedStyle, "fontSize">>): this {
    Object.assign(this.data, opts);
    return this;
  }

  color(c: Color): this {
    this.data.color = c;
    return this;
  }

  padding(v: number): this {
    this.data.padding = v;
    return this;
  }

  width(v: number): this {
    this.data.width = v;
    return this;
  }

  height(v: number): this {
    this.data.height = v;
    return this;
  }

  resolve(): ResolvedStyle {
    return { ...this.data };
  }
}

export function style(): Style {
  return new Style();
}
