import { createSignal } from "solid-js";
import type { TurNodeHandle } from "./tur";

export interface TextControllerOptions {
  onInput?: (value: string) => void;
  onEnter?: () => void;
  onSelectionChange?: (start: number, end: number) => void;
}

export class TextController {
  private _value = createSignal("");
  private _cursorPosition = createSignal(0);
  private _focused = createSignal(false);
  private _selectionStart = createSignal(0);
  private _selectionEnd = createSignal(0);
  private _handle: TurNodeHandle | null = null;
  private _options?: TextControllerOptions;

  constructor(options?: TextControllerOptions) {
    this._options = options;
  }

  get value(): string {
    return this._value[0]();
  }
  get cursorPosition(): number {
    return this._cursorPosition[0]();
  }
  get focused(): boolean {
    return this._focused[0]();
  }
  get selectionStart(): number {
    return this._selectionStart[0]();
  }
  get selectionEnd(): number {
    return this._selectionEnd[0]();
  }
  get hasSelection(): boolean {
    return this._selectionStart[0]() !== this._selectionEnd[0]();
  }

  setText(text: string): void {
    if (this._handle) {
      __tur.setInputText(__tur.__ctx, this._handle, text);
      this._value[1](text);
      this._cursorPosition[1](text.length);
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
    this._value[1](text);
    this._options?.onInput?.(text);
    if (enter) this._options?.onEnter?.();
  }

  _onFocus(): void {
    this._focused[1](true);
  }

  _onBlur(): void {
    this._focused[1](false);
    this._cursorPosition[1](this._value[0]().length);
  }

  _onCursorChange(pos: number): void {
    this._cursorPosition[1](pos);
  }

  _onSelectionChange(start: number, end: number): void {
    this._selectionStart[1](start);
    this._selectionEnd[1](end);
    this._options?.onSelectionChange?.(start, end);
  }
}
