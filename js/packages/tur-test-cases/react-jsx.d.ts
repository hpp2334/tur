import type {
    Axis,
    BoxFit,
    Color,
    CrossAxisAlignment,
    FlexDirection,
    FlexFit,
    HitTestBehavior,
    MainAxisAlignment,
    ResourceHandle,
    StackFit,
    TurKeyEvent,
    TurNodeHandle,
} from "@tur/react-renderer";

declare module "react/jsx-runtime" {
    namespace JSX {
        interface IntrinsicElements {
            tur_flex: {
                direction?: FlexDirection;
                mainAlignment?: MainAxisAlignment;
                crossAlignment?: CrossAxisAlignment;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_flex_item: {
                flex?: number;
                fit?: FlexFit;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_stack: {
                fit?: StackFit;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_positioned: {
                left?: number;
                top?: number;
                right?: number;
                bottom?: number;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_container: {
                width?: number;
                height?: number;
                padding?: number;
                color?: Color;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_pointer_interact: {
                onClick?: () => void;
                onPointerEnter?: () => void;
                onPointerExit?: () => void;
                behavior?: HitTestBehavior;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_focusable: {
                ref?: (el: TurNodeHandle) => void;
                onFocus?: () => void;
                onBlur?: () => void;
                onKeyDown?: (e: TurKeyEvent) => boolean | void;
                onKeyUp?: (e: TurKeyEvent) => boolean | void;
                queryKey?: string[];
                children?: React.ReactNode;
            };
            tur_editable_text: {
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
            tur_paragraph: {
                fontSize?: number;
                spans?: {
                    content?: string;
                    bold?: boolean;
                    italic?: boolean;
                    underline?: boolean;
                    fontSize?: number;
                    color?: Color;
                }[];
                queryKey?: string[];
            };
            tur_image: {
                resourceId: ResourceHandle;
                width?: number;
                height?: number;
                fit?: BoxFit;
                queryKey?: string[];
            };
            tur_svg: {
                resourceId: ResourceHandle;
                width?: number;
                height?: number;
                fit?: BoxFit;
                queryKey?: string[];
            };
            tur_scroll_view: {
                axis?: Axis;
                controller?: object;
                queryKey?: string[];
                children?: React.ReactNode;
                ref?: (el: TurNodeHandle) => void;
            };
            tur_lazy_list: {
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
