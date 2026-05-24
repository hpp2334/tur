import type { TurKeyEvent, TextControllerHandle } from "./tur";

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
  private _controllerHandle: TextControllerHandle;
  private _options?: InputControllerOptions;

  constructor(options?: InputControllerOptions) {
    this._options = options;
    this._controllerHandle = __tur.createTextController(__tur.__ctx);
  }

  get text(): string {
    return __tur.textControllerText(__tur.__ctx, this._controllerHandle);
  }

  get controllerHandle(): TextControllerHandle {
    return this._controllerHandle;
  }

  requestFocus(): void {
    if (this._handle) {
      __tur.requestFocus(__tur.__ctx, this._handle);
    }
  }

  setSpans(spans: Array<{ content?: string; bold?: boolean; italic?: boolean; underline?: boolean; fontSize?: number; color?: unknown }>): void {
    __tur.textControllerSetSpans(__tur.__ctx, this._controllerHandle, spans);
  }

  clear(): void {
    __tur.textControllerClear(__tur.__ctx, this._controllerHandle);
  }

  _attach(h: object): void {
    this._handle = h;
  }

  _onInput(text: string, enter: boolean): void {
    this._options?.onInput?.(text, enter);
    if (enter) {
      this._options?.onEnter?.();
    }
  }

  _onCursorChange(position: number): void {
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
