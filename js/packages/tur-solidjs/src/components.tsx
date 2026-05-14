import type { JSX } from "solid-js";
import type { Color, ResourceHandle, TurKeyEvent } from "@tur/solidjs-renderer";
import type { InputController } from "@tur/solidjs-renderer";
import { BoxFit, CrossAxisAlignment, FlexDirection, MainAxisAlignment } from "@tur/solidjs-renderer";
import type { StackFit } from "@tur/solidjs-renderer";
import type { FlexFit } from "@tur/solidjs-renderer";
import { createElement, insert, setProp, spread, mergeProps } from "@tur/solidjs-renderer";

declare const __tur: {
  __ctx: unknown;
  getFirstChild: (ctx: unknown, node: unknown) => unknown;
  removeChild: (ctx: unknown, parent: unknown, child: unknown) => void;
};

interface BaseProps {
  children?: JSX.Element;
  queryKey?: string[];
}

export interface ColumnProps extends BaseProps {
  mainAlignment?: MainAxisAlignment;
  crossAlignment?: CrossAxisAlignment;
}

export interface RowProps extends BaseProps {
  mainAlignment?: MainAxisAlignment;
  crossAlignment?: CrossAxisAlignment;
}

export interface ExpandedProps extends BaseProps {
  flex?: number;
  fit?: FlexFit;
}

export interface StackProps extends BaseProps {
  fit?: StackFit;
}

export interface PositionedProps extends BaseProps {
  left?: number;
  top?: number;
  right?: number;
  bottom?: number;
}

export interface SizedBoxProps extends BaseProps {
  width?: number;
  height?: number;
}

export interface ContainerProps extends BaseProps {
  width?: number;
  height?: number;
  padding?: number;
  color?: Color;
}

export interface PointerInteractProps {
  onClick?: () => void;
  child?: JSX.Element;
}

export interface FocusableProps {
  onFocus?: () => void;
  onBlur?: () => void;
  onKeyDown?: (e: TurKeyEvent) => boolean | void;
  onKeyUp?: (e: TurKeyEvent) => boolean | void;
  child?: JSX.Element;
}

export interface TextProps extends BaseProps {
  content: string;
  fontSize?: number;
  color?: Color;
}

export function Column(props: ColumnProps): JSX.Element {
  const { crossAlignment = CrossAxisAlignment.Stretch, ...rest } = props;
  const el = createElement("tur_flex");
  setProp(el, "crossAlignment", crossAlignment);
  spread(el, mergeProps({ get direction() { return FlexDirection.Vertical; } }, rest), true);
  let init = false;
  insert(el, () => {
    if (init) {
      const c = __tur.__ctx;
      let child: unknown;
      while ((child = __tur.getFirstChild(c, el)) != null) {
        __tur.removeChild(c, el, child);
      }
    }
    init = true;
    return props.children;
  });
  return el as unknown as JSX.Element;
}

export function Row(props: RowProps): JSX.Element {
  const { children, mainAlignment = MainAxisAlignment.Start, crossAlignment = CrossAxisAlignment.Stretch, ...rest } = props;
  return <tur_flex direction={FlexDirection.Horizontal} mainAlignment={mainAlignment} crossAlignment={crossAlignment} {...rest}>{children}</tur_flex>;
}

export function Expanded(props: ExpandedProps): JSX.Element {
  const { children, ...rest } = props;
  return <tur_flex_item {...rest}>{children}</tur_flex_item>;
}

export function Stack(props: StackProps): JSX.Element {
  const { children, ...rest } = props;
  return <tur_stack {...rest}>{children}</tur_stack>;
}

export function Positioned(props: PositionedProps): JSX.Element {
  const { children, ...rest } = props;
  return <tur_positioned {...rest}>{children}</tur_positioned>;
}

export function SizedBox(props: SizedBoxProps): JSX.Element {
  const { children, ...rest } = props;
  return <tur_container {...rest}>{children}</tur_container>;
}

export function Container(props: ContainerProps): JSX.Element {
  const { children, ...rest } = props;
  return <tur_container {...rest}>{children}</tur_container>;
}

export function PointerInteract(props: PointerInteractProps): JSX.Element {
  const { child, ...rest } = props;
  return <tur_pointer_interact {...rest}>{child}</tur_pointer_interact>;
}

export function Focusable(props: FocusableProps): JSX.Element {
  const { child, ...rest } = props;
  return <tur_focusable {...rest}>{child}</tur_focusable>;
}

export interface TextContainerProps extends BaseProps {
  fontSize?: number;
  color?: Color | string;
}

export interface TextSpanProps {
  content?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  fontSize?: number;
  color?: Color | string;
}

export function TextContainer(props: TextContainerProps): JSX.Element {
  const { children, ...rest } = props;
  return <tur_text_container {...rest}>{children}</tur_text_container>;
}

export function TextSpan(props: TextSpanProps): JSX.Element {
  return <tur_text_span {...props} />;
}

export function Text(props: TextProps): JSX.Element {
  return (
    <TextContainer fontSize={props.fontSize} color={props.color} queryKey={props.queryKey}>
      <TextSpan content={props.content} />
    </TextContainer>
  );
}

export interface InputProps {
  controller: InputController;
  placeholder?: string;
  fontSize?: number;
  color?: Color;
  placeholderColor?: Color;
  cursorColor?: Color;
  multiline?: boolean;
  width?: number;
  height?: number;
}

export function Input(props: InputProps): JSX.Element {
  const ctrl = props.controller;
  return (
    <tur_container width={props.width} height={props.height}>
      <tur_input
        ref={(el: import("@tur/solidjs-renderer").TurNodeHandle) => ctrl._attach(el)}
        onInput={(text: string, enter: boolean) => ctrl._onInput(text, enter)}
        onKeyDown={(e: import("@tur/solidjs-renderer").TurKeyEvent) => ctrl._onKeyDown(e)}
        onFocus={() => ctrl._onFocus()}
        onBlur={() => ctrl._onBlur()}
        onCursorChange={(pos: number) => ctrl._onCursorChange(pos)}
        onSelectionChange={(start: number, end: number) => ctrl._onSelectionChange(start, end)}
        onCompositionStart={() => ctrl._onCompositionStart()}
        onCompositionUpdate={(text: string) => ctrl._onCompositionUpdate(text)}
        onCompositionEnd={(text: string) => ctrl._onCompositionEnd(text)}
        placeholder={props.placeholder}
        fontSize={props.fontSize ?? 14}
        color={props.color}
        placeholderColor={props.placeholderColor}
        cursorColor={props.cursorColor ?? props.color}
        multiline={props.multiline}
      />
    </tur_container>
  );
}

export interface ImageProps {
  resource: ResourceHandle;
  width?: number;
  height?: number;
  fit?: BoxFit;
  queryKey?: string[];
}

export function Image(props: ImageProps): JSX.Element {
  return (
    <tur_image
      resourceId={props.resource}
      width={props.width}
      height={props.height}
      fit={props.fit ?? BoxFit.Contain}
      queryKey={props.queryKey}
    />
  );
}
