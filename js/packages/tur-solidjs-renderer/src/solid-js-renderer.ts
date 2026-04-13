import { createRenderer } from "solid-js/universal";
import type { TurWidgetAPI } from "./types";

function getTurWidget(): TurWidgetAPI {
  return (globalThis as any).tur.widget;
}

class SolidJsRenderer {
  renderer: ReturnType<typeof createRenderer>;

  constructor() {
    this.renderer = createRenderer({
      createElement(type: string): any {
        return getTurWidget().createElement(type);
      },

      createTextNode(value: string): any {
        const handle = getTurWidget().createElement("Text");
        getTurWidget().setAttribute(handle, "content", String(value));
        return handle;
      },

      replaceText(textNode: any, value: string): void {
        getTurWidget().setAttribute(textNode, "content", value);
      },

      setProperty(node: any, name: string, value: any): void {
        if (typeof value === "object" && value !== null) value = String(value);
        getTurWidget().setAttribute(node, name, value);
      },

      insertNode(parent: any, node: any, anchor?: any): void {
        if (anchor != null) {
          getTurWidget().insertBefore(parent, node, anchor);
        } else {
          getTurWidget().appendChild(parent, node);
        }
      },

      removeNode(parent: any, node: any): void {
        getTurWidget().removeChild(parent, node);
      },

      getParentNode(node: any): any {
        return getTurWidget().getParent(node);
      },

      getFirstChild(node: any): any {
        return getTurWidget().getFirstChild(node);
      },

      getNextSibling(node: any): any {
        return getTurWidget().getNextSibling(node);
      },
    });
  }
}

export const solidRenderer = new SolidJsRenderer();
