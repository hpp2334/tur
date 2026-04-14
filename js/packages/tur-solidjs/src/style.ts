import { bridge } from "./bridge";
import type { Color } from "./generated/Color";
import type { CrossAxisAlignment } from "./generated/CrossAxisAlignment";
import type { MainAxisAlignment } from "./generated/MainAxisAlignment";

interface FlexCategory {
  mainAlignment: MainAxisAlignment | null;
  crossAlignment: CrossAxisAlignment | null;
  gap: number | null;
}

interface TextCategory {
  fontSize: { value: number; unit: string } | null;
}

interface StyleInternal {
  flex: FlexCategory | null;
  text: TextCategory | null;
  color: Color | null;
  padding: number | null;
  width: number | null;
  height: number | null;
}

export const Flex = {
  gap(value: number): Partial<FlexCategory> {
    return { gap: value };
  },
  mainAlignment(v: MainAxisAlignment): Partial<FlexCategory> {
    return { mainAlignment: v };
  },
  crossAlignment(v: CrossAxisAlignment): Partial<FlexCategory> {
    return { crossAlignment: v };
  },
};

export const TextOpts = {
  fontSize(value: number, unit = "px"): { fontSize: { value: number; unit: string } } {
    return { fontSize: { value, unit } };
  },
};

export class Style {
  private data: StyleInternal = {
    flex: null,
    text: null,
    color: null,
    padding: null,
    width: null,
    height: null,
  };

  flex(opts: Partial<FlexCategory>): this {
    if (!this.data.flex) {
      this.data.flex = { mainAlignment: null, crossAlignment: null, gap: null };
    }
    Object.assign(this.data.flex, opts);
    return this;
  }

  text(opts: Partial<TextCategory>): this {
    if (!this.data.text) {
      this.data.text = { fontSize: null };
    }
    Object.assign(this.data.text, opts);
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

  apply(handle: number): void {
    const widget = bridge();
    if (this.data.flex) {
      if (this.data.flex.mainAlignment != null)
        widget.setAttribute(handle, "mainAlignment", this.data.flex.mainAlignment);
      if (this.data.flex.crossAlignment != null)
        widget.setAttribute(handle, "crossAlignment", this.data.flex.crossAlignment);
      if (this.data.flex.gap != null)
        widget.setAttribute(handle, "gap", this.data.flex.gap);
    }
    if (this.data.text?.fontSize) {
      widget.setAttribute(handle, "fontSize", this.data.text.fontSize.value);
    }
    if (this.data.color != null) {
      widget.setAttribute(handle, "color", String(this.data.color));
    }
    if (this.data.padding != null) {
      widget.setAttribute(handle, "padding", this.data.padding);
    }
    if (this.data.width != null) {
      widget.setAttribute(handle, "width", this.data.width);
    }
    if (this.data.height != null) {
      widget.setAttribute(handle, "height", this.data.height);
    }
  }
}

export function style(): Style {
  return new Style();
}
