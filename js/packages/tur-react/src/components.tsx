import React from "react";
import type { ReactNode, Ref } from "react";
import type { Color, ResourceHandle, TurKeyEvent, TurNodeHandle } from "@tur/react-renderer";
import type { InputController } from "@tur/react-renderer";
import { BoxFit, CrossAxisAlignment, FlexDirection, MainAxisAlignment } from "@tur/react-renderer";
import type { StackFit } from "@tur/react-renderer";
import type { FlexFit } from "@tur/react-renderer";

interface BaseProps {
  children?: ReactNode;
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
  child?: ReactNode;
}

export interface FocusableProps {
  ref?: Ref<TurNodeHandle>;
  onFocus?: () => void;
  onBlur?: () => void;
  onKeyDown?: (e: TurKeyEvent) => boolean | void;
  onKeyUp?: (e: TurKeyEvent) => boolean | void;
  child?: ReactNode;
}

export interface TextProps extends BaseProps {
  content: string;
  fontSize?: number;
  color?: Color;
}

export function Column(props: ColumnProps) {
  const { children, crossAlignment, mainAlignment, queryKey, ...rest } = props;
  return (
    <tur_flex
      direction={FlexDirection.Vertical}
      crossAlignment={crossAlignment ?? CrossAxisAlignment.Stretch}
      mainAlignment={mainAlignment}
      queryKey={queryKey}
      {...rest}
    >
      {children}
    </tur_flex>
  );
}

export function Row(props: RowProps) {
  const { children, mainAlignment, crossAlignment, queryKey, ...rest } = props;
  return (
    <tur_flex
      direction={FlexDirection.Horizontal}
      mainAlignment={mainAlignment ?? MainAxisAlignment.Start}
      crossAlignment={crossAlignment ?? CrossAxisAlignment.Stretch}
      queryKey={queryKey}
      {...rest}
    >
      {children}
    </tur_flex>
  );
}

export function Expanded(props: ExpandedProps) {
  return <tur_flex_item flex={props.flex} fit={props.fit} queryKey={props.queryKey}>{props.children}</tur_flex_item>;
}

export function Stack(props: StackProps) {
  return <tur_stack fit={props.fit} queryKey={props.queryKey}>{props.children}</tur_stack>;
}

export function Positioned(props: PositionedProps) {
  return <tur_positioned left={props.left} top={props.top} right={props.right} bottom={props.bottom} queryKey={props.queryKey}>{props.children}</tur_positioned>;
}

export function SizedBox(props: SizedBoxProps) {
  return <tur_container width={props.width} height={props.height} queryKey={props.queryKey}>{props.children}</tur_container>;
}

export function Container(props: ContainerProps) {
  return <tur_container width={props.width} height={props.height} padding={props.padding} color={props.color} queryKey={props.queryKey}>{props.children}</tur_container>;
}

export function PointerInteract(props: PointerInteractProps) {
  return <tur_pointer_interact onClick={props.onClick}>{props.child}</tur_pointer_interact>;
}

export function Focusable(props: FocusableProps) {
  const { ref, onFocus, onBlur, onKeyDown, onKeyUp, child } = props;
  return <tur_focusable ref={ref} onFocus={onFocus} onBlur={onBlur} onKeyDown={onKeyDown} onKeyUp={onKeyUp}>{child}</tur_focusable>;
}

export interface TextContainerProps extends BaseProps {
  fontSize?: number;
}

export interface TextSpanProps {
  content?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  fontSize?: number;
  color?: Color | string;
}

export function TextContainer(props: TextContainerProps) {
  return <tur_text_container fontSize={props.fontSize} queryKey={props.queryKey}>{props.children}</tur_text_container>;
}

export function TextSpan(props: TextSpanProps) {
  return <tur_text_span {...props} color={props.color ?? "#000000"} />;
}

export function Text(props: TextProps) {
  return (
    <TextContainer fontSize={props.fontSize} queryKey={props.queryKey}>
      <TextSpan content={props.content} color={props.color} />
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

export function Input(props: InputProps) {
  const ctrl = props.controller;
  return (
    <tur_container width={props.width} height={props.height}>
      <tur_input
        ref={(el: TurNodeHandle) => ctrl._attach(el)}
        onInput={(text: string, enter: boolean) => ctrl._onInput(text, enter)}
        onKeyDown={(e: TurKeyEvent) => ctrl._onKeyDown(e)}
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

export function Image(props: ImageProps) {
  return <tur_image resourceId={props.resource} width={props.width} height={props.height} fit={props.fit ?? BoxFit.Contain} queryKey={props.queryKey} />;
}
