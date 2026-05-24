import type { Color, LinearGradient, ResourceHandle, TurKeyEvent, TurNodeHandle } from "@tur/react-renderer";
import type { BoxFit } from "@tur/react-renderer";
import type { BorderPosition } from "@tur/react-renderer";
import type { CrossAxisAlignment } from "@tur/react-renderer";
import type { FlexDirection } from "@tur/react-renderer";
import type { FlexFit } from "@tur/react-renderer";
import type { HitTestBehavior } from "@tur/react-renderer";
import type { MainAxisSize } from "@tur/react-renderer";
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
        mainAxisSize?: MainAxisSize;
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
        color?: Color | LinearGradient;
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
        onPointerEnter?: () => void;
        onPointerExit?: () => void;
        behavior?: HitTestBehavior;
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
      tur_editable_text: RefAttributes<TurNodeHandle> & {
        controller?: object;
        fontSize?: number;
        color?: Color;
        cursorColor?: Color;
        placeholder?: string;
        placeholderColor?: Color;
        multiline?: boolean;
        onInput?: (text: string, enter: boolean) => void;
        onCursorChange?: (position: number) => void;
        onSelectionChange?: (anchor: number, end: number) => void;
        onKeyDown?: (e: TurKeyEvent) => boolean | void;
        onKeyUp?: (e: TurKeyEvent) => boolean | void;
        onFocus?: () => void;
        onBlur?: () => void;
        onCompositionStart?: () => void;
        onCompositionUpdate?: (text: string) => void;
        onCompositionEnd?: (text: string) => void;
        ref?: (el: TurNodeHandle) => void;
      };
      tur_paragraph: RefAttributes<TurNodeHandle> & {
        fontSize?: number;
        spans?: { content?: string; bold?: boolean; italic?: boolean; underline?: boolean; fontSize?: number; color?: Color }[];
        onSelectionChange?: (anchor: number, end: number) => void;
        queryKey?: string[];
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
