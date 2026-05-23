import React from "react";
import type { ReactNode, Ref } from "react";
import { Color, LinearGradient } from "@tur/react-renderer";
import type { ResourceHandle, TurKeyEvent, TurNodeHandle } from "@tur/react-renderer";
import type { InputController } from "@tur/react-renderer";
import { BoxFit, CrossAxisAlignment, FlexDirection, MainAxisSize, MainAxisAlignment, HitTestBehavior } from "@tur/react-renderer";
import type { StackFit } from "@tur/react-renderer";
import type { FlexFit } from "@tur/react-renderer";
import type { BorderPosition } from "@tur/react-renderer";

interface BaseProps {
  children?: ReactNode;
  queryKey?: string[];
}

export interface ColumnProps extends BaseProps {
  mainAlignment?: MainAxisAlignment;
  crossAlignment?: CrossAxisAlignment;
  mainAxisSize?: MainAxisSize;
}

export interface RowProps extends BaseProps {
  mainAlignment?: MainAxisAlignment;
  crossAlignment?: CrossAxisAlignment;
  mainAxisSize?: MainAxisSize;
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
  color?: Color | LinearGradient;
  borderColor?: Color;
  borderWidth?: number;
  borderRadius?: number;
  borderPosition?: BorderPosition;
  shadowColor?: Color;
  shadowOffset?: [number, number];
  shadowBlur?: number;
}

export interface PointerInteractProps {
  onClick?: () => void;
  onPointerEnter?: () => void;
  onPointerExit?: () => void;
  behavior?: HitTestBehavior;
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

export function Column(props: ColumnProps) {
  const { children, crossAlignment, mainAlignment, mainAxisSize, queryKey, ...rest } = props;
  return (
    <tur_flex
      direction={FlexDirection.Vertical}
      crossAlignment={crossAlignment ?? CrossAxisAlignment.Stretch}
      mainAlignment={mainAlignment}
      mainAxisSize={mainAxisSize}
      queryKey={queryKey}
      {...rest}
    >
      {children}
    </tur_flex>
  );
}

export function Row(props: RowProps) {
  const { children, mainAlignment, crossAlignment, mainAxisSize, queryKey, ...rest } = props;
  return (
    <tur_flex
      direction={FlexDirection.Horizontal}
      mainAlignment={mainAlignment ?? MainAxisAlignment.Start}
      crossAlignment={crossAlignment ?? CrossAxisAlignment.Stretch}
      mainAxisSize={mainAxisSize}
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
  return <tur_container width={props.width} height={props.height} padding={props.padding} color={props.color} borderColor={props.borderColor} borderWidth={props.borderWidth} borderRadius={props.borderRadius} borderPosition={props.borderPosition} shadowColor={props.shadowColor} shadowOffset={props.shadowOffset} shadowBlur={props.shadowBlur} queryKey={props.queryKey}>{props.children}</tur_container>;
}

export function PointerInteract(props: PointerInteractProps) {
  return <tur_pointer_interact onClick={props.onClick} onPointerEnter={props.onPointerEnter} onPointerExit={props.onPointerExit} behavior={props.behavior}>{props.child}</tur_pointer_interact>;
}

export function Focusable(props: FocusableProps) {
  const { ref, onFocus, onBlur, onKeyDown, onKeyUp, child } = props;
  return <tur_focusable ref={ref} onFocus={onFocus} onBlur={onBlur} onKeyDown={onKeyDown} onKeyUp={onKeyUp}>{child}</tur_focusable>;
}

export interface ParagraphProps extends BaseProps {
  spans?: Array<{ content: string; bold?: boolean; italic?: boolean; underline?: boolean; fontSize?: number; color?: Color }>;
  fontSize?: number;
  onSelectionChange?: (anchor: number, end: number) => void;
}

export function Paragraph(props: ParagraphProps) {
  return <tur_paragraph spans={props.spans} fontSize={props.fontSize} onSelectionChange={props.onSelectionChange} queryKey={props.queryKey} />;
}

export interface TextProps extends BaseProps {
  content: string;
  fontSize?: number;
  color?: Color;
}

export function Text(props: TextProps) {
  return (
    <tur_paragraph
      spans={[{ content: props.content, color: props.color }]}
      fontSize={props.fontSize}
      queryKey={props.queryKey}
    />
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
  ctrl.setMultiline(!!props.multiline);
  return (
    <tur_container width={props.width} height={props.height}>
      <tur_editable_text
        ref={(el: TurNodeHandle) => ctrl._attach(el)}
        spans={ctrl.spans}
        placeholder={props.placeholder}
        fontSize={props.fontSize ?? 14}
        color={props.color}
        placeholderColor={props.placeholderColor}
        cursorColor={props.cursorColor ?? props.color}
        multiline={props.multiline}
        cursorPosition={ctrl.cursorPosition ?? undefined}
        selectionStart={ctrl.selectionStartProp ?? undefined}
        selectionEnd={ctrl.selectionEndProp ?? undefined}
        onKeyDown={(e: TurKeyEvent) => ctrl.handleKeyDown(e)}
        onPointerDown={(x: number, y: number) => ctrl.handlePointerDown(x, y)}
        onPointerMove={(x: number, y: number) => ctrl.handlePointerMove(x, y)}
        onCompositionStart={() => ctrl.handleCompositionStart()}
        onCompositionUpdate={(text: string) => ctrl.handleCompositionUpdate(text)}
        onCompositionEnd={(text: string) => ctrl.handleCompositionEnd(text)}
        onFocus={() => ctrl._onFocus()}
        onBlur={() => ctrl._onBlur()}
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
