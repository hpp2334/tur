import type { Color, ResourceHandle, TurKeyEvent, TurNodeHandle } from "@tur/react-renderer";
import type { BoxFit } from "@tur/react-renderer";
import type { CrossAxisAlignment } from "@tur/react-renderer";
import type { FlexDirection } from "@tur/react-renderer";
import type { FlexFit } from "@tur/react-renderer";
import type { MainAxisAlignment } from "@tur/react-renderer";
import type { StackFit } from "@tur/react-renderer";

declare module "react/jsx-runtime" {
  namespace JSX {
    interface IntrinsicElements {
      tur_flex: {
        direction?: FlexDirection;
        mainAlignment?: MainAxisAlignment;
        crossAlignment?: CrossAxisAlignment;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_flex_item: {
        flex?: number;
        fit?: FlexFit;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_stack: {
        fit?: StackFit;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_positioned: {
        left?: number;
        top?: number;
        right?: number;
        bottom?: number;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_container: {
        width?: number;
        height?: number;
        padding?: number;
        color?: Color;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_pointer_interact: {
        onClick?: () => void;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_focusable: {
        ref?: (el: TurNodeHandle) => void;
        onFocus?: () => void;
        onBlur?: () => void;
        onKeyDown?: (e: TurKeyEvent) => boolean | void;
        onKeyUp?: (e: TurKeyEvent) => boolean | void;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_input: {
        ref?: (el: TurNodeHandle) => void;
        onInput?: (value: string, enterPressed: boolean) => void;
        onFocus?: () => void;
        onBlur?: () => void;
        onKeyDown?: (e: TurKeyEvent) => void;
        onCursorChange?: (position: number) => void;
        onSelectionChange?: (start: number, end: number) => void;
        onCompositionStart?: () => void;
        onCompositionUpdate?: (text: string) => void;
        onCompositionEnd?: (text: string) => void;
        fontSize?: number;
        color?: Color;
        cursorColor?: Color;
        placeholder?: string;
        placeholderColor?: Color;
        multiline?: boolean;
      };
      tur_text_container: {
        fontSize?: number;
        color?: Color | string;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_text_span: {
        content?: string;
        bold?: boolean;
        italic?: boolean;
        underline?: boolean;
        fontSize?: number;
        color?: Color | string;
      };
      tur_image: {
        resourceId: ResourceHandle;
        width?: number;
        height?: number;
        fit?: BoxFit;
        queryKey?: string[];
      };
    }
  }
}

export {};
