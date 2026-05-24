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

export interface TextCursorRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

declare class TextEditingController {
  get text(): string;
  get cursorPosition(): number;
  get selectionAnchor(): number;
  get selectionEnd(): number;
  setSpans(spans: Array<{content?: string; bold?: boolean; italic?: boolean; underline?: boolean; fontSize?: number; color?: unknown}>): void;
  setSelection(anchor: number, end: number): void;
  clear(): void;
  _attach(handle: TurNodeHandle): void;
  requestFocus(ctx: unknown): void;
}

interface TextEditingControllerOptions {
  onInput?: (text: string, enter: boolean) => void;
  onFocus?: () => void;
  onBlur?: () => void;
  onKeyDown?: (e: TurKeyEvent) => void;
  onKeyUp?: (e: TurKeyEvent) => void;
  onCursorChange?: (pos: number) => void;
  onSelectionChange?: (start: number, end: number) => void;
  onCompositionStart?: () => void;
  onCompositionUpdate?: (text: string) => void;
  onCompositionEnd?: (text: string) => void;
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
    createEditableText(ctx: unknown, controller: TextEditingController): TurNodeHandle;
    createImage(ctx: unknown): TurNodeHandle;
    createImageResource(ctx: unknown, data: Uint8Array): ResourceHandle;
    createRoot(ctx: unknown): TurNodeHandle;
    createTextEditingController(ctx: unknown, options?: TextEditingControllerOptions): TextEditingController;
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
