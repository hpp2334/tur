import { createRenderer, type Renderer } from "solid-js/universal";
import type { TurNodeHandle } from "./tur";
import type { JSX } from "solid-js";

export type TurElement = TurNodeHandle;

const ctx = __tur.__ctx;

const creators: Record<string, () => TurElement> = {
  "tur_flex": () => __tur.createFlex(ctx),
  "tur_flex_item": () => __tur.createFlexItem(ctx),
  "tur_stack": () => __tur.createStack(ctx),
  "tur_positioned": () => __tur.createPositioned(ctx),
  "tur_container": () => __tur.createContainer(ctx),
  "tur_text_container": () => __tur.createTextContainer(ctx),
  "tur_text_span": () => __tur.createTextSpan(ctx),
  "tur_pointer_interact": () => __tur.createPointerInteract(ctx),
  "tur_focusable": () => __tur.createFocusable(ctx),
  "tur_input": () => __tur.createInput(ctx),
};

const _r = createRenderer<TurElement>({
  createElement(type: string): TurElement {
    const create = creators[type];
    if (!create) throw new Error(`unknown element type: ${type}`);
    return create();
  },

  createTextNode(_value: string): TurElement {
    throw new Error("createTextNode is not supported; use <tur_text_span> instead");
  },

  replaceText(_textNode: TurElement, _value: string): void {},

  isTextNode(_node: TurElement): boolean {
    return false;
  },

  setProperty<T>(node: TurElement, name: string, value: T): void {
    if (name === "ref" && typeof value === "function") {
      (value as (el: TurElement) => void)(node);
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
export const insertNode = _r.insertNode;
