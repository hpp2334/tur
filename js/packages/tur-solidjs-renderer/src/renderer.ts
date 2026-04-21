import { createRenderer, type Renderer } from "solid-js/universal";
import type { TurNodeHandle } from "./tur";
import type { ResolvedStyle } from "./style";
import type { JSX } from "solid-js";

export type TurElement = TurNodeHandle;

const ctx = __tur.__ctx;

const creators: Record<string, () => TurElement> = {
  "tur_flex": () => __tur.createFlex(ctx),
  "tur_flex_item": () => __tur.createFlexItem(ctx),
  "tur_stack": () => __tur.createStack(ctx),
  "tur_positioned": () => __tur.createPositioned(ctx),
  "tur_container": () => __tur.createContainer(ctx),
  "tur_text": () => __tur.createText(ctx),
  "tur_pointer_interact": () => __tur.createPointerInteract(ctx),
};

const _r: Renderer<TurElement> = createRenderer<TurElement>({
  createElement(type: string): TurElement {
    const create = creators[type];
    if (!create) throw new Error(`unknown element type: ${type}`);
    return create();
  },

  createTextNode(value: string): TurElement {
    const handle = __tur.createText(ctx);
    __tur.setAttribute(ctx, handle, "content", value);
    return handle;
  },

  replaceText(textNode: TurElement, value: string): void {
    __tur.setAttribute(ctx, textNode, "content", value);
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
          __tur.setAttribute(ctx, node, key, v);
        }
      }
      return;
    }
    __tur.setAttribute(ctx, node, name, value);
  },

  insertNode(parent: TurElement, node: TurElement, anchor?: TurElement): void {
    if (anchor != null) {
      __tur.insertBefore(ctx, parent, node, anchor);
    } else {
      __tur.appendChild(ctx, parent, node);
    }
  },

  removeNode(parent: TurElement, node: TurElement): void {
    __tur.removeChild(ctx, parent, node);
  },

  getParentNode(node: TurElement): TurElement | undefined {
    return __tur.getParent(ctx, node) ?? undefined;
  },

  getFirstChild(node: TurElement): TurElement | undefined {
    return __tur.getFirstChild(ctx, node) ?? undefined;
  },

  getNextSibling(node: TurElement): TurElement | undefined {
    return __tur.getNextSibling(ctx, node) ?? undefined;
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
  const root = __tur.createRoot(ctx);
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
