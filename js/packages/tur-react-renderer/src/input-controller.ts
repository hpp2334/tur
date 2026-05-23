import type { TurKeyEvent } from "./tur";

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
  private _handle: object | null = null;
  private _options?: InputControllerOptions;
  private _text: string = "";
  private _cursorPosition: number = 0;

  constructor(options?: InputControllerOptions) {
    this._options = options;
  }

  get text(): string {
    return this._text;
  }

  get cursorPosition(): number {
    return this._cursorPosition;
  }

  requestFocus(): void {
    if (this._handle) {
      __tur.requestFocus(__tur.__ctx, this._handle);
    }
  }

  clear(): void {
    this._text = "";
    this._cursorPosition = 0;
  }

  _attach(h: object): void {
    this._handle = h;
  }

  _onInput(text: string, enter: boolean): void {
    this._text = text;
    this._options?.onInput?.(text, enter);
    if (enter) {
      this._options?.onEnter?.();
    }
  }

  _onCursorChange(position: number): void {
    this._cursorPosition = position;
    this._options?.onCursorChange?.(position);
  }

  _onSelectionChange(anchor: number, end: number): void {
    this._options?.onSelectionChange?.(anchor, end);
  }

  _onCompositionStart(): void {
    this._options?.onCompositionStart?.();
  }

  _onCompositionUpdate(text: string): void {
    this._options?.onCompositionUpdate?.(text);
  }

  _onCompositionEnd(text: string): void {
    this._options?.onCompositionEnd?.(text);
  }

  _onFocus(): void {
    this._options?.onFocus?.();
  }

  _onBlur(): void {
    this._options?.onBlur?.();
  }

  _onKeyDown(e: TurKeyEvent): void {
    this._options?.onKeyDown?.(e);
  }
}
