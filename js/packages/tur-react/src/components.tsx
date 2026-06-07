import type {
    Alignment as AlignmentType,
    Axis,
    BorderPosition,
    FlexFit,
    ResourceHandle,
    ScrollController,
    StackFit,
    TextEditingController,
    TurKeyEvent,
    TurNodeHandle,
} from "@tur/react-renderer";
import {
    Alignment,
    BoxFit,
    type Color,
    CrossAxisAlignment,
    FlexDirection,
    type HitTestBehavior,
    type LinearGradient,
    MainAxisAlignment,
    type MainAxisSize,
} from "@tur/react-renderer";
import type { ReactNode, Ref } from "react";
import React from "react";

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
    alignment?: AlignmentType;
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
    alignment?: AlignmentType;
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
    const {
        children,
        crossAlignment,
        mainAlignment,
        mainAxisSize,
        queryKey,
        ...rest
    } = props;
    return (
        <tur_flex
            direction={FlexDirection.Vertical}
            crossAlignment={crossAlignment ?? CrossAxisAlignment.Center}
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
    const {
        children,
        mainAlignment,
        crossAlignment,
        mainAxisSize,
        queryKey,
        ...rest
    } = props;
    return (
        <tur_flex
            direction={FlexDirection.Horizontal}
            mainAlignment={mainAlignment ?? MainAxisAlignment.Start}
            crossAlignment={crossAlignment ?? CrossAxisAlignment.Center}
            mainAxisSize={mainAxisSize}
            queryKey={queryKey}
            {...rest}
        >
            {children}
        </tur_flex>
    );
}

export function Expanded(props: ExpandedProps) {
    return (
        <tur_flex_item
            flex={props.flex}
            fit={props.fit}
            queryKey={props.queryKey}
        >
            {props.children}
        </tur_flex_item>
    );
}

export function Stack(props: StackProps) {
    return (
        <tur_stack fit={props.fit} alignment={props.alignment} queryKey={props.queryKey}>
            {props.children}
        </tur_stack>
    );
}

export function Positioned(props: PositionedProps) {
    return (
        <tur_positioned
            left={props.left}
            top={props.top}
            right={props.right}
            bottom={props.bottom}
            queryKey={props.queryKey}
        >
            {props.children}
        </tur_positioned>
    );
}

export function SizedBox(props: SizedBoxProps) {
    return (
        <tur_container
            width={props.width}
            height={props.height}
            queryKey={props.queryKey}
        >
            {props.children}
        </tur_container>
    );
}

export function Container(props: ContainerProps) {
    return (
        <tur_container
            width={props.width}
            height={props.height}
            padding={props.padding}
            color={props.color}
            borderColor={props.borderColor}
            borderWidth={props.borderWidth}
            borderRadius={props.borderRadius}
            borderPosition={props.borderPosition}
            shadowColor={props.shadowColor}
            shadowOffset={props.shadowOffset}
            shadowBlur={props.shadowBlur}
            alignment={props.alignment}
            queryKey={props.queryKey}
        >
            {props.children}
        </tur_container>
    );
}

export function PointerInteract(props: PointerInteractProps) {
    return (
        <tur_pointer_interact
            onClick={props.onClick}
            onPointerEnter={props.onPointerEnter}
            onPointerExit={props.onPointerExit}
            behavior={props.behavior}
        >
            {props.child}
        </tur_pointer_interact>
    );
}

export function Focusable(props: FocusableProps) {
    const { ref, onFocus, onBlur, onKeyDown, onKeyUp, child } = props;
    return (
        <tur_focusable
            ref={ref}
            onFocus={onFocus}
            onBlur={onBlur}
            onKeyDown={onKeyDown}
            onKeyUp={onKeyUp}
        >
            {child}
        </tur_focusable>
    );
}

export interface ParagraphProps extends BaseProps {
    spans?: Array<{
        content: string;
        bold?: boolean;
        italic?: boolean;
        underline?: boolean;
        fontSize?: number;
        color?: Color;
    }>;
    fontSize?: number;
    onSelectionChange?: (anchor: number, end: number) => void;
}

export function Paragraph(props: ParagraphProps) {
    return (
        <tur_paragraph
            spans={props.spans}
            fontSize={props.fontSize}
            onSelectionChange={props.onSelectionChange}
            queryKey={props.queryKey}
        />
    );
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
    controller: TextEditingController;
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
            <tur_editable_text
                ref={(el: TurNodeHandle) => ctrl._attach(el)}
                controller={ctrl}
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

export interface SvgProps {
    resource: ResourceHandle;
    width?: number;
    height?: number;
    fit?: BoxFit;
    queryKey?: string[];
}

export function Svg(props: SvgProps) {
    return (
        <tur_svg
            resourceId={props.resource}
            width={props.width}
            height={props.height}
            fit={props.fit ?? BoxFit.Contain}
            queryKey={props.queryKey}
        />
    );
}

export interface ScrollViewProps extends BaseProps {
    axis?: Axis;
    controller?: ScrollController;
}

export function ScrollView(props: ScrollViewProps) {
    const { controller, axis, children, queryKey } = props;
    return (
        <tur_scroll_view
            ref={
                controller
                    ? (el: TurNodeHandle) => controller._attach(el, __tur.__ctx)
                    : undefined
            }
            controller={controller}
            axis={axis}
            queryKey={queryKey}
        >
            {children}
        </tur_scroll_view>
    );
}
