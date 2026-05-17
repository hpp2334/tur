import type { Color, ResourceHandle, TurKeyEvent, TurNodeHandle } from "@tur/react-renderer";
import type { BoxFit } from "@tur/react-renderer";
import type { BorderPosition } from "@tur/react-renderer";
import type { CrossAxisAlignment } from "@tur/react-renderer";
import type { FlexDirection } from "@tur/react-renderer";
import type { FlexFit } from "@tur/react-renderer";
import type { MainAxisAlignment } from "@tur/react-renderer";
import type { StackFit } from "@tur/react-renderer";
import type { RefAttributes } from "react";

declare module "react/jsx-runtime" {
  namespace JSX {
    interface IntrinsicElements {
      tur_flex: RefAttributes<TurNodeHandle> & {
        direction?: FlexDirection;
        mainAlignment?: MainAxisAlignment;
        crossAlignment?: CrossAxisAlignment;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_flex_item: RefAttributes<TurNodeHandle> & {
        flex?: number;
        fit?: FlexFit;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_stack: RefAttributes<TurNodeHandle> & {
        fit?: StackFit;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_positioned: RefAttributes<TurNodeHandle> & {
        left?: number;
        top?: number;
        right?: number;
        bottom?: number;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_container: RefAttributes<TurNodeHandle> & {
        width?: number;
        height?: number;
        padding?: number;
        color?: Color;
        borderColor?: Color;
        borderWidth?: number;
        borderRadius?: number;
        borderPosition?: BorderPosition;
        shadowColor?: Color;
        shadowOffset?: [number, number];
        shadowBlur?: number;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_pointer_interact: RefAttributes<TurNodeHandle> & {
        onClick?: () => void;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_focusable: RefAttributes<TurNodeHandle> & {
        onFocus?: () => void;
        onBlur?: () => void;
        onKeyDown?: (e: TurKeyEvent) => boolean | void;
        onKeyUp?: (e: TurKeyEvent) => boolean | void;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_input: RefAttributes<TurNodeHandle> & {
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
      tur_text_container: RefAttributes<TurNodeHandle> & {
        fontSize?: number;
        color?: Color;
        queryKey?: string[];
        children?: React.ReactNode;
      };
      tur_text_span: RefAttributes<TurNodeHandle> & {
        content?: string;
        bold?: boolean;
        italic?: boolean;
        underline?: boolean;
        fontSize?: number;
        color?: Color;
      };
      tur_image: RefAttributes<TurNodeHandle> & {
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
