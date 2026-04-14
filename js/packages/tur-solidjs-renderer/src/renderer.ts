import { solidRenderer } from "./solid-js-renderer";
import type { TurWidgetAPI } from "./types";
import type { JSX } from "solid-js";

declare global {
  var tur: {
    widget: TurWidgetAPI;
  };
}

class _TurRenderer {
  render(component: () => JSX.Element): number {
    const root: number = globalThis.tur.widget.createElement("Column");
    const renderFn = solidRenderer.renderer.render.bind(solidRenderer.renderer);
    const createCompFn = solidRenderer.renderer.createComponent.bind(solidRenderer.renderer);
    renderFn(
      () => createCompFn(component as (props: Record<string, never>) => number, {}),
      root,
    );
    return root;
  }
}

export const TurRenderer: _TurRenderer = new _TurRenderer();
