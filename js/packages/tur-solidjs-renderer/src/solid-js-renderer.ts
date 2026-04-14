import { createRenderer, type Renderer } from "solid-js/universal";
import type { TurWidgetAPI } from "./types";

declare global {
  var tur: {
    widget: TurWidgetAPI;
  };
}

function getTurWidget(): TurWidgetAPI {
  return globalThis.tur.widget;
}

class SolidJsRenderer {
  renderer: Renderer<number>;

  constructor() {
    this.renderer = createRenderer<number>({
      createElement(type: string): number {
        return getTurWidget().createElement(type);
      },

      createTextNode(value: string): number {
        const handle = getTurWidget().createElement("Text");
        getTurWidget().setAttribute(handle, "content", String(value));
        return handle;
      },

      replaceText(textNode: number, value: string): void {
        getTurWidget().setAttribute(textNode, "content", value);
      },

      isTextNode(_node: number): boolean {
        return false;
      },

      setProperty<T>(node: number, name: string, value: T): void {
        const v = typeof value === "object" && value !== null ? String(value) : value;
        getTurWidget().setAttribute(node, name, v as string | number | boolean);
      },

      insertNode(parent: number, node: number, anchor?: number): void {
        if (anchor != null) {
          getTurWidget().insertBefore(parent, node, anchor);
        } else {
          getTurWidget().appendChild(parent, node);
        }
      },

      removeNode(parent: number, node: number): void {
        getTurWidget().removeChild(parent, node);
      },

      getParentNode(node: number): number | undefined {
        return getTurWidget().getParent(node) ?? undefined;
      },

      getFirstChild(node: number): number | undefined {
        return getTurWidget().getFirstChild(node) ?? undefined;
      },

      getNextSibling(node: number): number | undefined {
        return getTurWidget().getNextSibling(node) ?? undefined;
      },
    });
  }
}

export const solidRenderer = new SolidJsRenderer();
