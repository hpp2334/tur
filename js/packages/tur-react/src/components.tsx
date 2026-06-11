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
    createAnimationController,
    FlexDirection,
    type HitTestBehavior,
    type LinearGradient,
    MainAxisAlignment,
    type MainAxisSize,
} from "@tur/react-renderer";
import { flushSync } from "@tur/react-renderer";
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
        <tur_stack
            fit={props.fit}
            alignment={props.alignment}
            queryKey={props.queryKey}
        >
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

export const Container = React.forwardRef<TurNodeHandle, ContainerProps>(
    (props, ref) => {
        return (
            <tur_container
                ref={ref}
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
    },
);

export interface AnimatedContainerProps {
    children?: ReactNode;
    duration?: number;
    curve?: "linear" | "easeIn" | "easeOut" | "easeInOut";
    repeatCount?: number;
    onEnd?: () => void;
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
    queryKey?: string[];
}

type TweenEntry =
    | {
          begin: number;
          end: number;
          type: "float";
      }
    | {
          begin: { r: number; g: number; b: number; a: number };
          end: { r: number; g: number; b: number; a: number };
          type: "color";
      };

const ANIMATABLE_FLOAT_KEYS = [
    "width",
    "height",
    "padding",
    "borderWidth",
    "borderRadius",
    "shadowBlur",
] as const;

const ANIMATABLE_COLOR_KEYS = ["color", "borderColor", "shadowColor"] as const;

export function AnimatedContainer({
    duration = 300,
    curve = "linear",
    repeatCount,
    onEnd,
    children,
    ...containerProps
}: AnimatedContainerProps) {
    const [animatedValues, setAnimatedValues] = React.useState<
        Record<string, unknown>
    >({});
    const controllerRef = React.useRef<ReturnType<
        typeof createAnimationController
    > | null>(null);
    const tweenMapRef = React.useRef<Record<string, TweenEntry>>({});
    const prevPropsRef = React.useRef<typeof containerProps | null>(null);

    if (!controllerRef.current) {
        controllerRef.current = createAnimationController({
            duration,
            curve,
            onTick: (value: number) => {
                const tweens = tweenMapRef.current;
                const keys = Object.keys(tweens);
                if (keys.length === 0) return;
                const newValues: Record<string, unknown> = {};
                for (const [key, tween] of Object.entries(tweens)) {
                    if (tween.type === "float") {
                        newValues[key] =
                            tween.begin + (tween.end - tween.begin) * value;
                    } else {
                        const lerp = (a: number, b: number) =>
                            Math.round(a + (b - a) * value);
                        newValues[key] = {
                            type: "solid",
                            r: lerp(tween.begin.r, tween.end.r),
                            g: lerp(tween.begin.g, tween.end.g),
                            b: lerp(tween.begin.b, tween.end.b),
                            a: lerp(tween.begin.a, tween.end.a),
                        };
                    }
                }
                flushSync(() =>
                    setAnimatedValues((prev) => ({ ...prev, ...newValues })),
                );
            },
            onEnd,
        });
    }

    React.useLayoutEffect(() => {
        const ctrl = controllerRef.current;
        const prev = prevPropsRef.current;
        if (!ctrl || !prev) {
            prevPropsRef.current = containerProps;
            return;
        }

        const tweens: Record<string, TweenEntry> = {};

        for (const key of ANIMATABLE_FLOAT_KEYS) {
            const newVal = (containerProps as Record<string, unknown>)[key] as
                | number
                | undefined;
            const oldVal = (prev as Record<string, unknown>)[key] as
                | number
                | undefined;
            if (
                newVal !== undefined &&
                oldVal !== undefined &&
                newVal !== oldVal
            ) {
                tweens[key] = { begin: oldVal, end: newVal, type: "float" };
            }
        }

        for (const key of ANIMATABLE_COLOR_KEYS) {
            const newVal = (containerProps as Record<string, unknown>)[key] as
                | Color
                | undefined;
            const oldVal = (prev as Record<string, unknown>)[key] as
                | Color
                | undefined;
            if (
                newVal &&
                oldVal &&
                typeof newVal === "object" &&
                typeof oldVal === "object" &&
                (newVal.r !== oldVal.r ||
                    newVal.g !== oldVal.g ||
                    newVal.b !== oldVal.b ||
                    (newVal.a ?? 1) !== (oldVal.a ?? 1))
            ) {
                tweens[key] = {
                    begin: {
                        r: oldVal.r,
                        g: oldVal.g,
                        b: oldVal.b,
                        a: oldVal.a ?? 1,
                    },
                    end: {
                        r: newVal.r,
                        g: newVal.g,
                        b: newVal.b,
                        a: newVal.a ?? 1,
                    },
                    type: "color",
                };
            }
        }

        const tweenKeys = Object.keys(tweens);
        if (tweenKeys.length > 0) {
            tweenMapRef.current = tweens;
            if (repeatCount !== undefined) {
                ctrl.repeat(repeatCount);
            }
            ctrl.forward();
        }

        prevPropsRef.current = containerProps;
    });

    return (
        <tur_container
            {...(containerProps as Record<string, unknown>)}
            {...animatedValues}
        >
            {children}
        </tur_container>
    );
}

export interface AnimatedPositionedProps {
    children?: ReactNode;
    duration?: number;
    curve?: "linear" | "easeIn" | "easeOut" | "easeInOut";
    repeatCount?: number;
    onEnd?: () => void;
    left?: number;
    top?: number;
    right?: number;
    bottom?: number;
    queryKey?: string[];
}

export function AnimatedPositioned({
    duration = 300,
    curve = "linear",
    repeatCount,
    onEnd,
    children,
    ...positionedProps
}: AnimatedPositionedProps) {
    const [animatedValues, setAnimatedValues] = React.useState<
        Record<string, unknown>
    >({});
    const controllerRef = React.useRef<ReturnType<
        typeof createAnimationController
    > | null>(null);
    const tweenMapRef = React.useRef<
        Record<string, { begin: number; end: number }>
    >({});
    const prevPropsRef = React.useRef<typeof positionedProps | null>(null);

    if (!controllerRef.current) {
        controllerRef.current = createAnimationController({
            duration,
            curve,
            onTick: (value: number) => {
                const newValues: Record<string, unknown> = {};
                for (const [key, { begin, end }] of Object.entries(
                    tweenMapRef.current,
                )) {
                    newValues[key] = begin + (end - begin) * value;
                }
                flushSync(() =>
                    setAnimatedValues((prev) => ({ ...prev, ...newValues })),
                );
            },
            onEnd,
        });
    }

    React.useLayoutEffect(() => {
        const ctrl = controllerRef.current;
        const prev = prevPropsRef.current;
        if (!ctrl || !prev) {
            prevPropsRef.current = positionedProps;
            return;
        }

        const tweens: Record<string, { begin: number; end: number }> = {};
        for (const key of ["left", "top", "right", "bottom"] as const) {
            const newVal = positionedProps[key];
            const oldVal = prev[key];
            if (
                newVal !== undefined &&
                oldVal !== undefined &&
                newVal !== oldVal
            ) {
                tweens[key] = { begin: oldVal, end: newVal };
            }
        }

        if (Object.keys(tweens).length > 0) {
            tweenMapRef.current = tweens;
            if (repeatCount !== undefined) {
                ctrl.repeat(repeatCount);
            }
            ctrl.forward();
        }

        prevPropsRef.current = positionedProps;
    });

    return (
        <tur_positioned
            {...(positionedProps as Record<string, unknown>)}
            {...animatedValues}
        >
            {children}
        </tur_positioned>
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
