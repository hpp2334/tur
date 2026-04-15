export interface TurApp {
  createElement(type: string): number;
  createRoot(): number;
  setAttribute(handle: number, key: string, value: unknown): void;
  appendChild(parent: number, child: number): void;
  removeChild(parent: number, child: number): void;
  insertBefore(parent: number, child: number, ref: number): void;
  getParent(handle: number): number | null;
  getFirstChild(handle: number): number | null;
  getNextSibling(handle: number): number | null;
}

export function createTurApp(): TurApp {
  return (globalThis as any).createTurApp();
}
