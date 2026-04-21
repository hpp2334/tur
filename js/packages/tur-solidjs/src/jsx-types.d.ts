import type { ResolvedStyle } from "@tur/solidjs-renderer";
import type { Color } from "@tur/solidjs-renderer";
import type { CrossAxisAlignment } from "@tur/solidjs-renderer";
import type { FlexDirection } from "@tur/solidjs-renderer";
import type { FlexFit } from "@tur/solidjs-renderer";
import type { MainAxisAlignment } from "@tur/solidjs-renderer";
import type { StackFit } from "@tur/solidjs-renderer";

declare module "solid-js" {
  namespace JSX {
    interface IntrinsicElements {
      tur_flex: {
        style?: ResolvedStyle;
        direction: FlexDirection;
        mainAlignment?: MainAxisAlignment;
        crossAlignment?: CrossAxisAlignment;
        children?: JSX.Element;
      };
      tur_flex_item: {
        style?: ResolvedStyle;
        flex?: number;
        fit?: FlexFit;
        children?: JSX.Element;
      };
      tur_stack: {
        style?: ResolvedStyle;
        fit?: StackFit;
        children?: JSX.Element;
      };
      tur_positioned: {
        style?: ResolvedStyle;
        left?: number;
        top?: number;
        right?: number;
        bottom?: number;
        children?: JSX.Element;
      };
      tur_container: {
        style?: ResolvedStyle;
        width?: number;
        height?: number;
        padding?: number;
        color?: Color;
        children?: JSX.Element;
      };
      tur_pointer_interact: {
        style?: ResolvedStyle;
        onClick?: () => void;
        children?: JSX.Element;
      };
      tur_text: {
        style?: ResolvedStyle;
        content?: string;
        fontSize?: number;
        color?: Color;
        children?: JSX.Element;
      };
    }
  }
}

export {};
