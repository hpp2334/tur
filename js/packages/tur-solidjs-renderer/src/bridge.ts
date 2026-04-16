export type TurAppContext = unknown;

export function createTurAppContext(): TurAppContext {
  return (globalThis as any).tur_createAppContext();
}

export function tur_createElement(ctx: TurAppContext, type: string): number {
  return (globalThis as any).tur_createElement(ctx, type);
}

export function tur_createRoot(ctx: TurAppContext): number {
  return (globalThis as any).tur_createRoot(ctx);
}

export function tur_setAttribute(
  ctx: TurAppContext,
  handle: number,
  key: string,
  value: unknown,
): void {
  (globalThis as any).tur_setAttribute(ctx, handle, key, value);
}

export function tur_appendChild(
  ctx: TurAppContext,
  parent: number,
  child: number,
): void {
  (globalThis as any).tur_appendChild(ctx, parent, child);
}

export function tur_removeChild(
  ctx: TurAppContext,
  parent: number,
  child: number,
): void {
  (globalThis as any).tur_removeChild(ctx, parent, child);
}

export function tur_insertBefore(
  ctx: TurAppContext,
  parent: number,
  child: number,
  ref: number,
): void {
  (globalThis as any).tur_insertBefore(ctx, parent, child, ref);
}

export function tur_getParent(
  ctx: TurAppContext,
  handle: number,
): number | null {
  return (globalThis as any).tur_getParent(ctx, handle);
}

export function tur_getFirstChild(
  ctx: TurAppContext,
  handle: number,
): number | null {
  return (globalThis as any).tur_getFirstChild(ctx, handle);
}

export function tur_getNextSibling(
  ctx: TurAppContext,
  handle: number,
): number | null {
  return (globalThis as any).tur_getNextSibling(ctx, handle);
}
