import { createRenderer, type Renderer } from "solid-js/universal";
import {
  getTurAppContext,
  tur_createElement,
  tur_createRoot,
  tur_setAttribute,
  tur_appendChild,
  tur_removeChild,
  tur_insertBefore,
  tur_getParent,
  tur_getFirstChild,
  tur_getNextSibling,
  type TurAppContext,
} from "./bridge";
import type { ResolvedStyle } from "./style";
import type { JSX } from "solid-js";

export interface TurElement {
  readonly h: number;
}

function el(h: number): TurElement {
  return { h };
}

const ctx: TurAppContext = getTurAppContext();

const _r: Renderer<TurElement> = createRenderer<TurElement>({
  createElement(type: string): TurElement {
    return el(tur_createElement(ctx, type));
  },

  createTextNode(value: string): TurElement {
    const handle = tur_createElement(ctx, "tur_text");
    tur_setAttribute(ctx, handle, "content", value);
    return el(handle);
  },

  replaceText(textNode: TurElement, value: string): void {
    tur_setAttribute(ctx, textNode.h, "content", value);
  },

  isTextNode(_node: TurElement): boolean {
    return false;
  },

  setProperty<T>(node: TurElement, name: string, value: T): void {
    if (name === "style" && value != null && typeof value === "object") {
      const rs = value as unknown as ResolvedStyle;
      const keys = Object.keys(rs) as (keyof ResolvedStyle)[];
      for (let i = 0; i < keys.length; i++) {
        const key = keys[i];
        const v = rs[key];
        if (v !== null) {
          tur_setAttribute(ctx, node.h, key, v);
        }
      }
      return;
    }
    tur_setAttribute(ctx, node.h, name, value);
  },

  insertNode(parent: TurElement, node: TurElement, anchor?: TurElement): void {
    if (anchor != null) {
      tur_insertBefore(ctx, parent.h, node.h, anchor.h);
    } else {
      tur_appendChild(ctx, parent.h, node.h);
    }
  },

  removeNode(parent: TurElement, node: TurElement): void {
    tur_removeChild(ctx, parent.h, node.h);
  },

  getParentNode(node: TurElement): TurElement | undefined {
    const result = tur_getParent(ctx, node.h);
    return result != null ? el(result) : undefined;
  },

  getFirstChild(node: TurElement): TurElement | undefined {
    const result = tur_getFirstChild(ctx, node.h);
    return result != null ? el(result) : undefined;
  },

  getNextSibling(node: TurElement): TurElement | undefined {
    const result = tur_getNextSibling(ctx, node.h);
    return result != null ? el(result) : undefined;
  },
});

type ComponentType = string | ((props: Record<string, unknown>) => TurElement);

function createComponent(type: ComponentType, props: Record<string, unknown> | null): TurElement {
  if (typeof type === "string") {
    const element = _r.createElement(type);
    if (props) {
      const keys = Object.keys(props);
      for (let i = 0; i < keys.length; i++) {
        const key = keys[i];
        if (key === "children") continue;
        const value = props[key];
        if (value !== undefined && value !== null) {
          _r.setProp(element, key, value);
        }
      }
      if ("children" in props) {
        _r.insert(element, props.children);
      }
    }
    return element;
  }
  return _r.createComponent(type, props ?? {});
}

export function renderRoot(component: () => JSX.Element): TurElement {
  const root = el(tur_createRoot(ctx));
  _r.render(
    () => _r.createComponent(component as unknown as (props: Record<string, never>) => TurElement, {}),
    root,
  );
  return root;
}

export const createElement = _r.createElement;
export { createComponent };
export const insert = _r.insert;
export const spread = _r.spread;
export const setProp = _r.setProp;
export const effect = _r.effect;
export const memo = _r.memo;
export const mergeProps = _r.mergeProps;
export const use = _r.use;
