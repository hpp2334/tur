export type TurNodeHandle = object;
export type ResourceHandle = number;

export interface TurKeyEvent {
  key: string;
  code: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  meta: boolean;
}

declare global {
  var __tur: {
    __ctx: unknown;
    createFlex(ctx: unknown): TurNodeHandle;
    createFlexItem(ctx: unknown): TurNodeHandle;
    createStack(ctx: unknown): TurNodeHandle;
    createPositioned(ctx: unknown): TurNodeHandle;
    createContainer(ctx: unknown): TurNodeHandle;
    createTextContainer(ctx: unknown): TurNodeHandle;
    createTextSpan(ctx: unknown): TurNodeHandle;
    createPointerInteract(ctx: unknown): TurNodeHandle;
    createFocusable(ctx: unknown): TurNodeHandle;
    createInput(ctx: unknown): TurNodeHandle;
    createImage(ctx: unknown): TurNodeHandle;
    createImageResource(ctx: unknown, data: Uint8Array): ResourceHandle;
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
    requestFocus(ctx: unknown, handle: TurNodeHandle): void;
    setInputText(ctx: unknown, handle: TurNodeHandle, text: string): void;
  };
}
