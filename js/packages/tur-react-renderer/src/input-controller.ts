import type { TurNodeHandle, TurKeyEvent } from "./tur";

const ctx = __tur.__ctx;

export interface InputControllerOptions {
  onInput?: (value: string, enter: boolean) => void;
  onEnter?: () => void;
  onFocus?: () => void;
  onBlur?: () => void;
  onKeyDown?: (e: TurKeyEvent) => void;
  onCursorChange?: (pos: number) => void;
  onSelectionChange?: (start: number, end: number) => void;
  onCompositionStart?: () => void;
  onCompositionUpdate?: (text: string) => void;
  onCompositionEnd?: (text: string) => void;
}

export class InputController {
  private _handle: TurNodeHandle | null = null;
  private _options?: InputControllerOptions;

  private _text: string = "";
  private _cursorPosition: number = 0;
  private _selectionAnchor: number = 0;
  private _selectionEnd: number = 0;
  private _multiline: boolean = false;

  private _compositionText: string | null = null;
  private _compositionStart: number = 0;

  constructor(options?: InputControllerOptions) {
    this._options = options;
  }

  get text(): string {
    return this._text;
  }

  get cursorPosition(): number | null {
    return this._cursorPosition;
  }

  get selectionStartProp(): number | null {
    if (this._selectionAnchor === this._selectionEnd) return null;
    return Math.min(this._selectionAnchor, this._selectionEnd);
  }

  get selectionEndProp(): number | null {
    if (this._selectionAnchor === this._selectionEnd) return null;
    return Math.max(this._selectionAnchor, this._selectionEnd);
  }

  get spans(): Array<{ content: string }> {
    return [{ content: this._text }];
  }

  setText(text: string): void {
    this._text = text;
    this._cursorPosition = text.length;
    this._selectionAnchor = this._cursorPosition;
    this._selectionEnd = this._cursorPosition;
    this._compositionText = null;
  }

  clear(): void {
    this.setText("");
  }

  requestFocus(): void {
    if (this._handle) {
      __tur.requestFocus(ctx, this._handle);
    }
  }

  setMultiline(multiline: boolean): void {
    this._multiline = multiline;
  }

  _attach(h: TurNodeHandle): void {
    this._handle = h;
  }

  private hasSelection(): boolean {
    return this._selectionAnchor !== this._selectionEnd;
  }

  private selectionRange(): [number, number] {
    const a = this._selectionAnchor;
    const b = this._selectionEnd;
    return a <= b ? [a, b] : [b, a];
  }

  private deleteSelection(): void {
    if (!this.hasSelection()) return;
    const [start, end] = this.selectionRange();
    this._text = this._text.slice(0, start) + this._text.slice(end);
    this._cursorPosition = start;
    this._selectionAnchor = start;
    this._selectionEnd = start;
  }

  private clearSelection(): void {
    this._selectionAnchor = this._cursorPosition;
    this._selectionEnd = this._cursorPosition;
  }

  handleKeyDown(e: TurKeyEvent): void {
    const prevText = this._text;
    const prevCursor = this._cursorPosition;
    const prevAnchor = this._selectionAnchor;
    const prevEnd = this._selectionEnd;

    let handled = false;

    switch (e.key) {
      case "Backspace": {
        if (this.hasSelection()) {
          this.deleteSelection();
          handled = true;
        } else if (this._cursorPosition > 0) {
          const prev = this._text.codePointAt(this._cursorPosition - 1)!;
          const charLen = String.fromCodePoint(prev).length;
          const start = this._cursorPosition - charLen;
          this._text = this._text.slice(0, start) + this._text.slice(this._cursorPosition);
          this._cursorPosition = start;
          this.clearSelection();
          handled = true;
        }
        break;
      }
      case "Delete": {
        if (this.hasSelection()) {
          this.deleteSelection();
          handled = true;
        } else if (this._cursorPosition < this._text.length) {
          const next = this._text.codePointAt(this._cursorPosition)!;
          const charLen = String.fromCodePoint(next).length;
          this._text = this._text.slice(0, this._cursorPosition) + this._text.slice(this._cursorPosition + charLen);
          this.clearSelection();
          handled = true;
        }
        break;
      }
      case "ArrowLeft": {
        if (e.shift) {
          const newPos = this.prevCharBoundary();
          if (!this.hasSelection()) this._selectionAnchor = this._cursorPosition;
          this._selectionEnd = newPos;
          this._cursorPosition = newPos;
        } else if (this.hasSelection()) {
          const [start] = this.selectionRange();
          this._cursorPosition = start;
          this.clearSelection();
        } else {
          this._cursorPosition = this.prevCharBoundary();
          this.clearSelection();
        }
        handled = true;
        break;
      }
      case "ArrowRight": {
        if (e.shift) {
          const newPos = this.nextCharBoundary();
          if (!this.hasSelection()) this._selectionAnchor = this._cursorPosition;
          this._selectionEnd = newPos;
          this._cursorPosition = newPos;
        } else if (this.hasSelection()) {
          const [, end] = this.selectionRange();
          this._cursorPosition = end;
          this.clearSelection();
        } else {
          this._cursorPosition = this.nextCharBoundary();
          this.clearSelection();
        }
        handled = true;
        break;
      }
      case "Home": {
        if (e.shift) {
          if (!this.hasSelection()) this._selectionAnchor = this._cursorPosition;
          this._selectionEnd = 0;
          this._cursorPosition = 0;
        } else {
          this._cursorPosition = 0;
          this.clearSelection();
        }
        handled = true;
        break;
      }
      case "End": {
        if (e.shift) {
          if (!this.hasSelection()) this._selectionAnchor = this._cursorPosition;
          this._selectionEnd = this._text.length;
          this._cursorPosition = this._text.length;
        } else {
          this._cursorPosition = this._text.length;
          this.clearSelection();
        }
        handled = true;
        break;
      }
      case "a": {
        if (e.ctrl || e.meta) {
          this._selectionAnchor = 0;
          this._selectionEnd = this._text.length;
          this._cursorPosition = this._text.length;
          handled = true;
        }
        break;
      }
      case "Enter": {
        if (this._multiline) {
          if (this.hasSelection()) this.deleteSelection();
          this._text = this._text.slice(0, this._cursorPosition) + "\n" + this._text.slice(this._cursorPosition);
          this._cursorPosition += 1;
          this.clearSelection();
          handled = true;
        } else {
          handled = true;
        }
        break;
      }
      default: {
        if (e.key.length === 1 && !e.ctrl && !e.meta && !this._compositionText) {
          if (this.hasSelection()) this.deleteSelection();
          this._text = this._text.slice(0, this._cursorPosition) + e.key + this._text.slice(this._cursorPosition);
          this._cursorPosition += e.key.length;
          this.clearSelection();
          handled = true;
        }
        break;
      }
    }

    if (!handled) return;

    this._options?.onKeyDown?.(e);

    if (this._text !== prevText) {
      const enter = e.key === "Enter" && !this._multiline;
      this._options?.onInput?.(this._text, enter);
    }
    if (this._cursorPosition !== prevCursor) {
      this._options?.onCursorChange?.(this._cursorPosition);
    }
    if (this._selectionAnchor !== prevAnchor || this._selectionEnd !== prevEnd) {
      this._options?.onSelectionChange?.(
        Math.min(this._selectionAnchor, this._selectionEnd),
        Math.max(this._selectionAnchor, this._selectionEnd),
      );
    }
  }

  handlePointerDown(x: number, y: number): void {
    if (!this._handle) return;
    const charIndex = __tur.getCharIndexAtPosition(ctx, this._handle, x, y);
    this._cursorPosition = charIndex;
    this._selectionAnchor = charIndex;
    this._selectionEnd = charIndex;
  }

  handlePointerMove(x: number, y: number): void {
    if (!this._handle) return;
    const charIndex = __tur.getCharIndexAtPosition(ctx, this._handle, x, y);
    this._selectionEnd = charIndex;
    this._cursorPosition = charIndex;
  }

  handleCompositionStart(): void {
    this._compositionText = "";
    this._compositionStart = this._cursorPosition;
    this._options?.onCompositionStart?.();
  }

  handleCompositionUpdate(text: string): void {
    this._compositionText = text;
    this._options?.onCompositionUpdate?.(text);
  }

  handleCompositionEnd(text: string): void {
    if (this._compositionText !== null) {
      const start = Math.min(this._compositionStart, this._text.length);
      this._text = this._text.slice(0, start) + text + this._text.slice(start);
      this._cursorPosition = start + text.length;
      this.clearSelection();
      this._compositionText = null;

      this._options?.onCompositionEnd?.(text);
      this._options?.onInput?.(this._text, false);
      this._options?.onCursorChange?.(this._cursorPosition);
    }
  }

  _onFocus(): void {
    this._options?.onFocus?.();
  }

  _onBlur(): void {
    this._options?.onBlur?.();
  }

  private prevCharBoundary(): number {
    let p = this._cursorPosition;
    while (p > 0) {
      p--;
      if (this._text.charCodeAt(p) < 0xd800 || this._text.charCodeAt(p) >= 0xdc00) {
        return p;
      }
    }
    return 0;
  }

  private nextCharBoundary(): number {
    let p = this._cursorPosition;
    while (p < this._text.length) {
      p++;
      if (p >= this._text.length || this._text.charCodeAt(p) < 0xd800 || this._text.charCodeAt(p) >= 0xdc00) {
        return p;
      }
    }
    return this._text.length;
  }
}
