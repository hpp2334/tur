import { solidRenderer } from "./solid-js-renderer";

class _TurRenderer {
  render(component: () => any): number {
    const root = (globalThis as any).tur.widget.createElement("Column");
    solidRenderer.renderer.render(
      () => solidRenderer.renderer.createComponent(component, {}),
      root,
    );
    return root;
  }
}

export const TurRenderer = new _TurRenderer();
