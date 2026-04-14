import { createRenderer, type Renderer } from "solid-js/universal";
import { bridge } from "@tur/solidjs";
import type { TurWidgetAPI } from "@tur/solidjs";

class SolidJsRenderer {
  renderer: Renderer<number>;

  constructor() {
    this.renderer = createRenderer<number>({
      createElement(type: string): number {
        return bridge().createElement(type);
      },

      createTextNode(value: string): number {
        const handle = bridge().createElement("Text");
        bridge().setAttribute(handle, "content", String(value));
        return handle;
      },

      replaceText(textNode: number, value: string): void {
        bridge().setAttribute(textNode, "content", value);
      },

      isTextNode(_node: number): boolean {
        return false;
      },

      setProperty<T>(node: number, name: string, value: T): void {
        const v = typeof value === "object" && value !== null ? String(value) : value;
        bridge().setAttribute(node, name, v as string | number | boolean);
      },

      insertNode(parent: number, node: number, anchor?: number): void {
        if (anchor != null) {
          bridge().insertBefore(parent, node, anchor);
        } else {
          bridge().appendChild(parent, node);
        }
      },

      removeNode(parent: number, node: number): void {
        bridge().removeChild(parent, node);
      },

      getParentNode(node: number): number | undefined {
        return bridge().getParent(node) ?? undefined;
      },

      getFirstChild(node: number): number | undefined {
        return bridge().getFirstChild(node) ?? undefined;
      },

      getNextSibling(node: number): number | undefined {
        return bridge().getNextSibling(node) ?? undefined;
      },
    });
  }
}

export const solidRenderer = new SolidJsRenderer();
