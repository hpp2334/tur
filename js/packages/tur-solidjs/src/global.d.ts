export interface TurWidgetAPI {
  createElement(type: string): number;
  setAttribute(handle: number, key: string, value: string | number | boolean): void;
  appendChild(parent: number, child: number): void;
  removeChild(parent: number, child: number): void;
  insertBefore(parent: number, child: number, ref: number): void;
  getParent(handle: number): number | null;
  getFirstChild(handle: number): number | null;
  getNextSibling(handle: number): number | null;
}

declare global {
  var tur: {
    widget: TurWidgetAPI;
  };
}
