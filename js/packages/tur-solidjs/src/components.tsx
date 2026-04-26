import type { JSX } from "solid-js";
import type { Color, TurKeyEvent } from "@tur/solidjs-renderer";
import { CrossAxisAlignment, FlexDirection, MainAxisAlignment } from "@tur/solidjs-renderer";
import type { StackFit } from "@tur/solidjs-renderer";
import type { FlexFit } from "@tur/solidjs-renderer";

interface BaseProps {
  children?: JSX.Element;
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
  const { children, crossAlignment = CrossAxisAlignment.Stretch, ...rest } = props;
  return <tur_flex direction={FlexDirection.Vertical} crossAlignment={crossAlignment} {...rest}>{children}</tur_flex>;
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

export function Text(props: TextProps): JSX.Element {
  const { children, ...rest } = props;
  return <tur_text {...rest}>{children}</tur_text>;
}
