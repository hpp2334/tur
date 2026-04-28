import type { Color, TurKeyEvent } from "@tur/solidjs-renderer";
import type { CrossAxisAlignment } from "@tur/solidjs-renderer";
import type { FlexDirection } from "@tur/solidjs-renderer";
import type { FlexFit } from "@tur/solidjs-renderer";
import type { MainAxisAlignment } from "@tur/solidjs-renderer";
import type { StackFit } from "@tur/solidjs-renderer";

declare module "solid-js" {
  namespace JSX {
    interface IntrinsicElements {
      tur_flex: {
        direction: FlexDirection;
        mainAlignment?: MainAxisAlignment;
        crossAlignment?: CrossAxisAlignment;
        queryKey?: string[];
        children?: JSX.Element;
      };
      tur_flex_item: {
        flex?: number;
        fit?: FlexFit;
        queryKey?: string[];
        children?: JSX.Element;
      };
      tur_stack: {
        fit?: StackFit;
        queryKey?: string[];
        children?: JSX.Element;
      };
      tur_positioned: {
        left?: number;
        top?: number;
        right?: number;
        bottom?: number;
        queryKey?: string[];
        children?: JSX.Element;
      };
      tur_container: {
        width?: number;
        height?: number;
        padding?: number;
        color?: Color;
        queryKey?: string[];
        children?: JSX.Element;
      };
      tur_pointer_interact: {
        onClick?: () => void;
        queryKey?: string[];
        ref?: (el: import("@tur/solidjs-renderer").TurNodeHandle) => void;
        children?: JSX.Element;
      };
      tur_focusable: {
        ref?: (el: import("@tur/solidjs-renderer").TurNodeHandle) => void;
        onFocus?: () => void;
        onBlur?: () => void;
        onKeyDown?: (e: TurKeyEvent) => boolean | void;
        onKeyUp?: (e: TurKeyEvent) => boolean | void;
        queryKey?: string[];
        children?: JSX.Element;
      };
      tur_input: {
        ref?: (el: import("@tur/solidjs-renderer").TurNodeHandle) => void;
        onInput?: (value: string, enterPressed: boolean) => void;
        onFocus?: () => void;
        onBlur?: () => void;
        onCursorChange?: (position: number) => void;
        fontSize?: number;
        color?: Color;
        cursorColor?: Color;
        placeholder?: string;
        placeholderColor?: Color;
      };
      tur_text_container: {
        fontSize?: number;
        color?: Color;
        queryKey?: string[];
        children?: JSX.Element;
      };
      tur_text_span: {
        content?: string;
        bold?: boolean;
        italic?: boolean;
        underline?: boolean;
        fontSize?: number;
        color?: Color;
      };
    }
  }
}

export {};
