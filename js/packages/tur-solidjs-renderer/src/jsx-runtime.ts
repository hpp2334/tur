import { solidRenderer } from "./solid-js-renderer";

const _r = solidRenderer.renderer;

function createComponent(type: any, props: any): any {
  if (typeof type === "string") {
    const el = _r.createElement(type);
    if (props) {
      const keys = Object.keys(props);
      for (let i = 0; i < keys.length; i++) {
        const key = keys[i];
        if (key === "children") continue;
        const value = props[key];
        if (value !== undefined && value !== null) {
          _r.setAttribute(el, key, value);
        }
      }
      if ("children" in props) {
        _r.insert(el, props.children);
      }
    }
    return el;
  }
  return _r.createComponent(type, props);
}

export const createElement = _r.createElement;
export { createComponent };
export const insert = _r.insert;
export const spread = _r.spread;
export const setAttribute = _r.setAttribute;
export const setProp = _r.setProp;
export const effect = _r.effect;
export const memo = _r.memo;
export const mergeProps = _r.mergeProps;
export const use = _r.use;

export const Column = "Column";
export const Row = "Row";
export const Text = "Text";
export const Container = "Container";
export const SizedBox = "SizedBox";
export const Expanded = "Expanded";
export const Stack = "Stack";
export const Positioned = "Positioned";
