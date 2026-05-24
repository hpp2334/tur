export type TurNodeHandle = object;
export type TextControllerHandle = object;
export type ResourceHandle = number;

export interface TurKeyEvent {
  key: string;
  code: string;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  meta: boolean;
}

export interface TextCursorRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

declare global {
  var __tur: {
    __ctx: unknown;
    createFlex(ctx: unknown): TurNodeHandle;
    createFlexItem(ctx: unknown): TurNodeHandle;
    createStack(ctx: unknown): TurNodeHandle;
    createPositioned(ctx: unknown): TurNodeHandle;
    createContainer(ctx: unknown): TurNodeHandle;
    createParagraph(ctx: unknown): TurNodeHandle;
    createPointerInteract(ctx: unknown): TurNodeHandle;
    createFocusable(ctx: unknown): TurNodeHandle;
    createEditableText(ctx: unknown, controller: TextControllerHandle): TurNodeHandle;
    createImage(ctx: unknown): TurNodeHandle;
    createImageResource(ctx: unknown, data: Uint8Array): ResourceHandle;
    createRoot(ctx: unknown): TurNodeHandle;
    createTextController(ctx: unknown): TextControllerHandle;
    textControllerSetSpans(ctx: unknown, handle: TextControllerHandle, spans: unknown[]): void;
    textControllerText(ctx: unknown, handle: TextControllerHandle): string;
    textControllerClear(ctx: unknown, handle: TextControllerHandle): void;
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
    getTextCursorRect(
      ctx: unknown,
      handle: TurNodeHandle,
      charIndex: number,
    ): TextCursorRect | null;
    getTextSelectionRects(
      ctx: unknown,
      handle: TurNodeHandle,
      start: number,
      end: number,
    ): Array<{ x: number; y: number; w: number; h: number }>;
    getCharIndexAtPosition(
      ctx: unknown,
      handle: TurNodeHandle,
      x: number,
      y: number,
    ): number;
  };
}
