import { solidRenderer } from "./solid-js-renderer";
import { bridge } from "@tur/solidjs";
import type { JSX } from "solid-js";

class _TurRenderer {
  render(component: () => JSX.Element): number {
    const root: number = bridge().createElement("Column");
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
