import type { JSX } from "solid-js";
import type { Style } from "./style";
import type { Color } from "./generated/Color";
import type { CrossAxisAlignment } from "./generated/CrossAxisAlignment";
import type { MainAxisAlignment } from "./generated/MainAxisAlignment";
import type { StackFit } from "./generated/StackFit";

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

export interface TextProps extends BaseProps {
  content: string;
  fontSize?: number;
  color?: Color;
}

function widget() {
  return globalThis.tur.widget;
}

function applyChildren(el: number, children: JSX.Element | undefined): void {
  if (children == null || children === false) return;
  if (Array.isArray(children)) {
    for (const child of children) {
      if (child != null && child !== false) {
        widget().appendChild(el, child as number);
      }
    }
  } else {
    widget().appendChild(el, children as number);
  }
}

function setAttr(el: number, key: string, value: string | number | boolean): void {
  widget().setAttribute(el, key, value);
}

export function Column(props: ColumnProps): JSX.Element {
  const el: number = widget().createElement("Column");
  if (props.mainAlignment != null) setAttr(el, "mainAlignment", props.mainAlignment);
  if (props.crossAlignment != null) setAttr(el, "crossAlignment", props.crossAlignment);
  props.style?.apply(el);
  applyChildren(el, props.children);
  return el;
}

export function Row(props: RowProps): JSX.Element {
  const el: number = widget().createElement("Row");
  if (props.mainAlignment != null) setAttr(el, "mainAlignment", props.mainAlignment);
  if (props.crossAlignment != null) setAttr(el, "crossAlignment", props.crossAlignment);
  props.style?.apply(el);
  applyChildren(el, props.children);
  return el;
}

export function Expanded(props: ExpandedProps): JSX.Element {
  const el: number = widget().createElement("Expanded");
  if (props.flex != null) setAttr(el, "flex", props.flex);
  props.style?.apply(el);
  applyChildren(el, props.children);
  return el;
}

export function Stack(props: StackProps): JSX.Element {
  const el: number = widget().createElement("Stack");
  if (props.fit != null) setAttr(el, "fit", props.fit);
  props.style?.apply(el);
  applyChildren(el, props.children);
  return el;
}

export function Positioned(props: PositionedProps): JSX.Element {
  const el: number = widget().createElement("Positioned");
  if (props.left != null) setAttr(el, "left", props.left);
  if (props.top != null) setAttr(el, "top", props.top);
  if (props.right != null) setAttr(el, "right", props.right);
  if (props.bottom != null) setAttr(el, "bottom", props.bottom);
  props.style?.apply(el);
  applyChildren(el, props.children);
  return el;
}

export function SizedBox(props: SizedBoxProps): JSX.Element {
  const el: number = widget().createElement("SizedBox");
  if (props.width != null) setAttr(el, "width", props.width);
  if (props.height != null) setAttr(el, "height", props.height);
  props.style?.apply(el);
  applyChildren(el, props.children);
  return el;
}

export function Container(props: ContainerProps): JSX.Element {
  const el: number = widget().createElement("Container");
  if (props.padding != null) setAttr(el, "padding", props.padding);
  if (props.color != null) setAttr(el, "color", String(props.color));
  props.style?.apply(el);
  applyChildren(el, props.children);
  return el;
}

export function Text(props: TextProps): JSX.Element {
  const el: number = widget().createElement("Text");
  setAttr(el, "content", props.content);
  if (props.fontSize != null) setAttr(el, "fontSize", props.fontSize);
  if (props.color != null) setAttr(el, "color", String(props.color));
  props.style?.apply(el);
  applyChildren(el, props.children);
  return el;
}
