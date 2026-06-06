import type {
    Axis,
    BorderPosition,
    BoxFit,
    Color,
    CrossAxisAlignment,
    FlexDirection,
    FlexFit,
    HitTestBehavior,
    LinearGradient,
    MainAxisAlignment,
    MainAxisSize,
    ResourceHandle,
    StackFit,
    TurKeyEvent,
    TurNodeHandle,
} from "@tur/react-renderer";
import type { RefAttributes } from "react";

declare module "react/jsx-runtime" {
    namespace JSX {
        interface IntrinsicElements {
            tur_flex: RefAttributes<TurNodeHandle> & {
                direction?: FlexDirection;
                mainAlignment?: MainAxisAlignment;
                crossAlignment?: CrossAxisAlignment;
                mainAxisSize?: MainAxisSize;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_flex_item: RefAttributes<TurNodeHandle> & {
                flex?: number;
                fit?: FlexFit;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_stack: RefAttributes<TurNodeHandle> & {
                fit?: StackFit;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_positioned: RefAttributes<TurNodeHandle> & {
                left?: number;
                top?: number;
                right?: number;
                bottom?: number;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_container: RefAttributes<TurNodeHandle> & {
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
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_pointer_interact: RefAttributes<TurNodeHandle> & {
                onClick?: () => void;
                onPointerEnter?: () => void;
                onPointerExit?: () => void;
                behavior?: HitTestBehavior;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_focusable: RefAttributes<TurNodeHandle> & {
                onFocus?: () => void;
                onBlur?: () => void;
                onKeyDown?: (e: TurKeyEvent) => boolean | void;
                onKeyUp?: (e: TurKeyEvent) => boolean | void;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_editable_text: RefAttributes<TurNodeHandle> & {
                controller?: object;
                fontSize?: number;
                color?: Color;
                cursorColor?: Color;
                placeholder?: string;
                placeholderColor?: Color;
                multiline?: boolean;
                onInput?: (text: string, enter: boolean) => void;
                onCursorChange?: (position: number) => void;
                onSelectionChange?: (anchor: number, end: number) => void;
                onKeyDown?: (e: TurKeyEvent) => boolean | void;
                onKeyUp?: (e: TurKeyEvent) => boolean | void;
                onFocus?: () => void;
                onBlur?: () => void;
                onCompositionStart?: () => void;
                onCompositionUpdate?: (text: string) => void;
                onCompositionEnd?: (text: string) => void;
                ref?: (el: TurNodeHandle) => void;
            };
            tur_paragraph: RefAttributes<TurNodeHandle> & {
                fontSize?: number;
                spans?: {
                    content?: string;
                    bold?: boolean;
                    italic?: boolean;
                    underline?: boolean;
                    fontSize?: number;
                    color?: Color;
                }[];
                onSelectionChange?: (anchor: number, end: number) => void;
                queryKey?: string[];
            };
            tur_image: RefAttributes<TurNodeHandle> & {
                resourceId: ResourceHandle;
                width?: number;
                height?: number;
                fit?: BoxFit;
                queryKey?: string[];
            };
            tur_svg: RefAttributes<TurNodeHandle> & {
                resourceId: ResourceHandle;
                width?: number;
                height?: number;
                fit?: BoxFit;
                queryKey?: string[];
            };
            tur_scroll_view: RefAttributes<TurNodeHandle> & {
                axis?: Axis;
                controller?: object;
                queryKey?: string[];
                children?: React.ReactNode;
                ref?: (el: TurNodeHandle) => void;
            };
            tur_lazy_list: RefAttributes<TurNodeHandle> & {
                axis?: Axis;
                itemCount?: number;
                overscan?: number;
                startIndex?: number;
                controller?: object;
                queryKey?: string[];
                children?: React.ReactNode;
                ref?: (el: TurNodeHandle) => void;
            };
        }
    }
}

export {};
