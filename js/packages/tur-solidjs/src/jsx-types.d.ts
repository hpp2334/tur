import type { ResolvedStyle } from "@tur/solidjs-renderer";
import type { Color } from "@tur/solidjs-renderer";
import type { CrossAxisAlignment } from "@tur/solidjs-renderer";
import type { MainAxisAlignment } from "@tur/solidjs-renderer";
import type { StackFit } from "@tur/solidjs-renderer";

declare module "solid-js" {
  namespace JSX {
    interface IntrinsicElements {
      tur_column: {
        style?: ResolvedStyle;
        mainAlignment?: MainAxisAlignment;
        crossAlignment?: CrossAxisAlignment;
        children?: JSX.Element;
      };
      tur_row: {
        style?: ResolvedStyle;
        mainAlignment?: MainAxisAlignment;
        crossAlignment?: CrossAxisAlignment;
        children?: JSX.Element;
      };
      tur_expanded: {
        style?: ResolvedStyle;
        flex?: number;
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
      tur_sized_box: {
        style?: ResolvedStyle;
        width?: number;
        height?: number;
        children?: JSX.Element;
      };
      tur_container: {
        style?: ResolvedStyle;
        padding?: number;
        color?: Color;
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
