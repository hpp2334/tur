export interface TurApp {
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
  function createTurApp(): TurApp;
}

let _app: TurApp | null = null;

export function bridge(): TurApp {
  if (!_app) {
    _app = createTurApp();
  }
  return _app;
}
