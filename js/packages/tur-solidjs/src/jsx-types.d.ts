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
        children?: JSX.Element;
      };
      tur_focusable: {
        onFocus?: () => void;
        onBlur?: () => void;
        onKeyDown?: (e: TurKeyEvent) => boolean | void;
        onKeyUp?: (e: TurKeyEvent) => boolean | void;
        queryKey?: string[];
        children?: JSX.Element;
      };
      tur_text: {
        content?: string;
        fontSize?: number;
        color?: Color;
        queryKey?: string[];
        children?: JSX.Element;
      };
    }
  }
}

export {};
