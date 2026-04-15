import { createRenderer, type Renderer } from "solid-js/universal";
import {
  createTurAppContext,
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

const ctx: TurAppContext = createTurAppContext();

const _r: Renderer<number> = createRenderer<number>({
  createElement(type: string): number {
    return tur_createElement(ctx, type);
  },

  createTextNode(value: string): number {
    const handle = tur_createElement(ctx, "tur_text");
    tur_setAttribute(ctx, handle, "content", value);
    return handle;
  },

  replaceText(textNode: number, value: string): void {
    tur_setAttribute(ctx, textNode, "content", value);
  },

  isTextNode(_node: number): boolean {
    return false;
  },

  setProperty<T>(node: number, name: string, value: T): void {
    if (name === "style" && value != null && typeof value === "object") {
      const rs = value as unknown as ResolvedStyle;
      const keys = Object.keys(rs) as (keyof ResolvedStyle)[];
      for (let i = 0; i < keys.length; i++) {
        const key = keys[i];
        const v = rs[key];
        if (v !== null) {
          tur_setAttribute(ctx, node, key, v);
        }
      }
      return;
    }
    tur_setAttribute(ctx, node, name, value);
  },

  insertNode(parent: number, node: number, anchor?: number): void {
    if (anchor != null) {
      tur_insertBefore(ctx, parent, node, anchor);
    } else {
      tur_appendChild(ctx, parent, node);
    }
  },

  removeNode(parent: number, node: number): void {
    tur_removeChild(ctx, parent, node);
  },

  getParentNode(node: number): number | undefined {
    return tur_getParent(ctx, node) ?? undefined;
  },

  getFirstChild(node: number): number | undefined {
    return tur_getFirstChild(ctx, node) ?? undefined;
  },

  getNextSibling(node: number): number | undefined {
    return tur_getNextSibling(ctx, node) ?? undefined;
  },
});

type ComponentType = string | ((props: Record<string, unknown>) => number);

function createComponent(type: ComponentType, props: Record<string, unknown> | null): number {
  if (typeof type === "string") {
    const el = _r.createElement(type);
    if (props) {
      const keys = Object.keys(props);
      for (let i = 0; i < keys.length; i++) {
        const key = keys[i];
        if (key === "children") continue;
        const value = props[key];
        if (value !== undefined && value !== null) {
          _r.setProp(el, key, value);
        }
      }
      if ("children" in props) {
        _r.insert(el, props.children);
      }
    }
    return el;
  }
  return _r.createComponent(type, props ?? {});
}

export function renderRoot(component: () => JSX.Element): number {
  const root: number = tur_createRoot(ctx);
  _r.render(
    () => _r.createComponent(component as (props: Record<string, never>) => number, {}),
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
