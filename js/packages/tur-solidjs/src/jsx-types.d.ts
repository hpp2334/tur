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
        direction: FlexDirection;
        mainAlignment?: MainAxisAlignment;
        crossAlignment?: CrossAxisAlignment;
        children?: JSX.Element;
      };
      tur_flex_item: {
        flex?: number;
        fit?: FlexFit;
        children?: JSX.Element;
      };
      tur_stack: {
        fit?: StackFit;
        children?: JSX.Element;
      };
      tur_positioned: {
        left?: number;
        top?: number;
        right?: number;
        bottom?: number;
        children?: JSX.Element;
      };
      tur_container: {
        width?: number;
        height?: number;
        padding?: number;
        color?: Color;
        children?: JSX.Element;
      };
      tur_pointer_interact: {
        onClick?: () => void;
        children?: JSX.Element;
      };
      tur_text: {
        content?: string;
        fontSize?: number;
        color?: Color;
        children?: JSX.Element;
      };
    }
  }
}

export {};
