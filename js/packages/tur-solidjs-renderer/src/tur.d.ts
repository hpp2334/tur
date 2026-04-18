export type TurNodeHandle = object;

declare global {
  var __tur: {
    __ctx: unknown;
    create(ctx: unknown, type: string): TurNodeHandle;
    createRoot(ctx: unknown): TurNodeHandle;
    setAttribute(ctx: unknown, handle: TurNodeHandle, key: string, value: unknown): void;
    appendChild(ctx: unknown, parent: TurNodeHandle, child: TurNodeHandle): void;
    removeChild(ctx: unknown, parent: TurNodeHandle, child: TurNodeHandle): void;
    insertBefore(
      ctx: unknown,
      parent: TurNodeHandle,
      child: TurNodeHandle,
      ref: TurNodeHandle,
    ): void;
    getParent(ctx: unknown, handle: TurNodeHandle): TurNodeHandle | null;
    getFirstChild(ctx: unknown, handle: TurNodeHandle): TurNodeHandle | null;
    getNextSibling(ctx: unknown, handle: TurNodeHandle): TurNodeHandle | null;
  };
}
