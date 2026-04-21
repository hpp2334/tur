import type { JSX } from "solid-js";
import type { Color } from "@tur/solidjs-renderer";
import type { CrossAxisAlignment } from "@tur/solidjs-renderer";
import type { MainAxisAlignment } from "@tur/solidjs-renderer";
import type { StackFit } from "@tur/solidjs-renderer";
import type { FlexFit } from "@tur/solidjs-renderer";
import { FlexDirection } from "@tur/solidjs-renderer";
import type { Style } from "./style";

interface BaseProps {
  style?: Style;
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
  padding?: number;
  color?: Color;
}

export interface PointerInteractProps {
  style?: Style;
  onClick?: () => void;
  child?: JSX.Element;
}

export interface TextProps extends BaseProps {
  content: string;
  fontSize?: number;
  color?: Color;
}

export function Column(props: ColumnProps): JSX.Element {
  const { style: s, children, ...rest } = props;
  return <tur_flex style={s?.resolve()} direction={FlexDirection.Vertical} {...rest}>{children}</tur_flex>;
}

export function Row(props: RowProps): JSX.Element {
  const { style: s, children, ...rest } = props;
  return <tur_flex style={s?.resolve()} direction={FlexDirection.Horizontal} {...rest}>{children}</tur_flex>;
}

export function Expanded(props: ExpandedProps): JSX.Element {
  const { style: s, children, ...rest } = props;
  return <tur_flex_item style={s?.resolve()} {...rest}>{children}</tur_flex_item>;
}

export function Stack(props: StackProps): JSX.Element {
  const { style: s, children, ...rest } = props;
  return <tur_stack style={s?.resolve()} {...rest}>{children}</tur_stack>;
}

export function Positioned(props: PositionedProps): JSX.Element {
  const { style: s, children, ...rest } = props;
  return <tur_positioned style={s?.resolve()} {...rest}>{children}</tur_positioned>;
}

export function SizedBox(props: SizedBoxProps): JSX.Element {
  const { style: s, children, ...rest } = props;
  return <tur_container style={s?.resolve()} {...rest}>{children}</tur_container>;
}

export function Container(props: ContainerProps): JSX.Element {
  const { style: s, children, ...rest } = props;
  return <tur_container style={s?.resolve()} {...rest}>{children}</tur_container>;
}

export function PointerInteract(props: PointerInteractProps): JSX.Element {
  const { style: s, child, ...rest } = props;
  return <tur_pointer_interact style={s?.resolve()} {...rest}>{child}</tur_pointer_interact>;
}

export function Text(props: TextProps): JSX.Element {
  const { style: s, children, ...rest } = props;
  return <tur_text style={s?.resolve()} {...rest}>{children}</tur_text>;
}
