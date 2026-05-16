import type { TurNodeHandle, TurKeyEvent } from "./tur";

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

  constructor(options?: InputControllerOptions) {
    this._options = options;
  }

  setText(text: string): void {
    if (this._handle) {
      __tur.setInputText(__tur.__ctx, this._handle, text);
    }
  }

  clear(): void {
    this.setText("");
  }

  requestFocus(): void {
    if (this._handle) {
      __tur.requestFocus(__tur.__ctx, this._handle);
    }
  }

  _attach(h: TurNodeHandle): void {
    this._handle = h;
  }

  _onInput(text: string, enter: boolean): void {
    this._options?.onInput?.(text, enter);
    if (enter) this._options?.onEnter?.();
  }

  _onFocus(): void {
    this._options?.onFocus?.();
  }

  _onBlur(): void {
    this._options?.onBlur?.();
  }

  _onCursorChange(pos: number): void {
    this._options?.onCursorChange?.(pos);
  }

  _onSelectionChange(start: number, end: number): void {
    this._options?.onSelectionChange?.(start, end);
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

  _onKeyDown(e: TurKeyEvent): void {
    this._options?.onKeyDown?.(e);
  }
}
