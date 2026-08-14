/**
 * @tur-ng/std — ambient type declarations for the native tur widget library.
 *
 * Runtime is a synthetic boa module registered by tur-engine under the
 * specifier `"tur:std"`. It re-exports everything from
 * `"tur:core"` (the reactive primitives + meta-types) and adds the
 * widget layer: view factories, prop interfaces, enums, value types (Color /
 * LinearGradient / SpanData), view controllers, resources, and the event
 * detail payloads.
 *
 * Consumers typically import from `tur:std` alone — it is the
 * convenience superset:
 * ```ts
 * import { Container, Column, source, Color, Axis } from "tur:std";
 * ```
 *
 * `@tur-ng/animation` and other libraries that need only the reactive
 * substrate may import directly from `tur:core`.
 */

declare module "tur:std" {
    // Re-export the reactive core (source/derive/mutate/get/set/view/render,
    // Element/Source/Derived/Mutation/Readable/Val, ReadonlyStoreCtx/StoreCtx).
    export * from "tur:core";

    // Core meta-types used by the prop interfaces below.
    import type { Element, Mutation, Readable, Val } from "tur:core";

    // ---------------------------------------------------------------------------
    // Value types — Color / LinearGradient / Brush / SpanData
    // ---------------------------------------------------------------------------

    /** A solid sRGB color handle (Rust `ColorOpaque`). Built via the `Color`
     *  builder's static methods (`Color.hex/rgb/rgba`); the runtime value is a
     *  Rust-owned opaque, so callers must treat it as opaque. `Color` is also the
     *  instance type (the handle returned by `createColor`). */
    export class Color {
        private constructor();
        static rgb(r: number, g: number, b: number): Color;
        static rgba(r: number, g: number, b: number, a: number): Color;
        static hex(hex: string): Color;
    }

    /** A gradient stop: an offset along the gradient and its color. */
    export interface GradientStop {
        offset: number;
        color: Color;
    }

    /** A linear gradient brush handle (Rust `BrushOpaque`). Built via
     *  `LinearGradient.create`. Opaque to JS. */
    export class LinearGradient {
        private constructor();
        static create(options: LinearGradientOptions): LinearGradient;
    }

    /** Options for `LinearGradient.create`. */
    export interface LinearGradientOptions {
        start: [number, number];
        end: [number, number];
        stops: GradientStop[];
    }

    /** Any fill the engine accepts for `color`-style props: a solid color or a
     *  gradient. */
    export type Brush = Color | LinearGradient;

    /** One styled run inside a rich-text `Text.spans` array. Mirrors the Rust
     *  `SpanData` struct (the JS field is `content`; Rust maps it to `text`). */
    export interface SpanData {
        content: string;
        /** CSS-style numeric font weight (100–1000). Omit for the default
         *  (400). Overrides the element's `fontWeight` for this run. */
        weight?: number;
        italic?: boolean;
        underline?: boolean;
        fontSize?: number;
        color?: Color;
    }

    /** The current canvas viewport size in CSS pixels — the value shape of the
     *  engine-owned `viewportSize$` reactive atom. The engine keeps it in sync
     *  on every resize; read via `get(viewportSize$).width`. */
    export interface ViewportSize {
        width: number;
        height: number;
    }

    /** Engine-owned reactive atom holding the live canvas size
     *  (`{width, height}` in CSS pixels). Updated each frame from the resize
     *  handler; import from `tur:std`. Read-only to app code — typed as a
     *  `Derived` so `set(viewportSize$, …)` is rejected at compile time. */
    export const viewportSize$: Derived<ViewportSize>;

    /** OS cursor keywords (CSS cursor names). Mirrors `tur_engine::core::platform::Cursor`. */
    export type Cursor =
        | "auto"
        | "default"
        | "none"
        | "context-menu"
        | "help"
        | "pointer"
        | "progress"
        | "wait"
        | "cell"
        | "crosshair"
        | "text"
        | "vertical-text"
        | "alias"
        | "copy"
        | "move"
        | "no-drop"
        | "not-allowed"
        | "grab"
        | "grabbing"
        | "e-resize"
        | "n-resize"
        | "ne-resize"
        | "nw-resize"
        | "s-resize"
        | "se-resize"
        | "sw-resize"
        | "w-resize"
        | "ew-resize"
        | "ns-resize"
        | "nesw-resize"
        | "nwse-resize"
        | "col-resize"
        | "row-resize"
        | "all-scroll"
        | "zoom-in"
        | "zoom-out";

    // ---------------------------------------------------------------------------
    // Event detail payloads
    // ---------------------------------------------------------------------------

    export interface Point {
        x: number;
        y: number;
    }

    export interface PointerInteractEvent {
        /** Position relative to the element's top-left. */
        local: Point;
        /** Position relative to the canvas. */
        global: Point;
    }

    export interface PointerRegionEvent {
        local: Point;
        global: Point;
    }

    export interface KeyEvent {
        key: string;
        code: string;
        ctrl: boolean;
        shift: boolean;
        alt: boolean;
        meta: boolean;
    }

    export interface ScrollEvent {
        offset: number;
        maxExtent: number;
        viewportDimension: number;
    }

    // ---------------------------------------------------------------------------
    // Enums — exported as runtime objects (`MainAxisSize.Max`) directly from
    // this native module; the matching type is the union of their literal values.
    // Mirrors the `tur_engine::core::layout` C-like enums. The native module exports each as a
    // TS-style numeric enum object (forward `Name: n` + reverse `"n": "Name"`).
    // ---------------------------------------------------------------------------

    export enum Axis {
        Vertical = 0,
        Horizontal = 1,
    }

    export enum MainAxisAlignment {
        Start = 0,
        Center = 1,
        End = 2,
        SpaceBetween = 3,
        SpaceAround = 4,
        SpaceEvenly = 5,
    }

    export enum CrossAxisAlignment {
        Start = 0,
        Center = 1,
        End = 2,
        Stretch = 3,
    }

    export enum MainAxisSize {
        Max = 0,
        Min = 1,
    }

    export enum HitTestBehavior {
        Opaque = 0,
        Translucent = 1,
    }

    export enum BoxFit {
        Fill = 0,
        Contain = 1,
        Cover = 2,
        FitWidth = 3,
        FitHeight = 4,
        None = 5,
    }

    export enum Alignment {
        TopLeft = 0,
        TopCenter = 1,
        TopRight = 2,
        CenterLeft = 3,
        Center = 4,
        CenterRight = 5,
        BottomLeft = 6,
        BottomCenter = 7,
        BottomRight = 8,
    }

    export enum BorderPosition {
        Inside = 0,
        Center = 1,
        Outside = 2,
    }

    export enum ClipBehavior {
        None = 0,
        HardEdge = 1,
        AntiAlias = 2,
        AntiAliasWithSaveLayer = 3,
    }

    // ---------------------------------------------------------------------------
    // Prop interfaces
    // ---------------------------------------------------------------------------

    export interface ContainerProps {
        width?: Val<number>;
        height?: Val<number>;
        padding?: Val<number>;
        color?: Val<Brush | null>;
        borderColor?: Val<Brush | null>;
        borderWidth?: Val<number>;
        borderRadius?: Val<number>;
        borderPosition?: Val<BorderPosition>;
        clipBehavior?: Val<ClipBehavior>;
        shadowColor?: Val<Brush | null>;
        shadowOffset?: Val<[number, number]>;
        shadowBlur?: Val<number>;
        alignment?: Val<Alignment>;
        queryKey?: Val<string[]>;
        children?: Element[];
    }

    export interface FlexProps {
        mainAlignment?: Val<MainAxisAlignment>;
        crossAlignment?: Val<CrossAxisAlignment>;
        mainAxisSize?: Val<MainAxisSize>;
        children: Element[];
    }

    export interface ExpandedProps {
        flex?: Val<number>;
        child: Element;
    }

    export interface StackProps {
        children: Element[];
    }

    export interface PositionedProps {
        left?: Val<number>;
        top?: Val<number>;
        right?: Val<number>;
        bottom?: Val<number>;
        width?: Val<number>;
        height?: Val<number>;
        child: Element;
    }

    export interface TextProps {
        text?: Val<string>;
        fontSize?: Val<number>;
        /** CSS-style numeric font weight (100–1000) applied to the whole
         *  element. Per-span `weight` overrides it for that range. Omit for
         *  the default (400). */
        fontWeight?: Val<number>;
        color?: Val<Brush | null>;
        spans?: Val<SpanData[]>;
        /** When `true`, the text can be drag-selected with the pointer. */
        selectable?: boolean;
        queryKey?: Val<string[]>;
        /**
         * Maximum number of lines to render. Ignored when `overflow` is
         * `"visible"`. When omitted (or `0`), the text wraps without limit.
         */
        maxLines?: Val<number>;
        /**
         * How content beyond `maxLines` is handled. Defaults to `"clip"`
         * when `maxLines` is set.
         * - `"clip"`     — render at most `maxLines` lines, discard the rest.
         * - `"ellipsis"` — render at most `maxLines` lines, appending `…`
         *                  to the last visible line (trimmed to fit).
         * - `"visible"`  — render all lines; `maxLines` is ignored.
         */
        overflow?: Val<TextOverflow>;
    }

    /**
     * How `Text` handles content beyond `maxLines`. Mirrors Flutter's
     * `TextOverflow`.
     */
    export type TextOverflow = "clip" | "ellipsis" | "visible";

    export interface PointerInteractProps {
        onClick?: Mutation<[PointerInteractEvent]>;
        onPointerDown?: Mutation<[PointerInteractEvent]>;
        onPointerMove?: Mutation<[PointerInteractEvent]>;
        onPointerUp?: Mutation<[PointerInteractEvent]>;
        onContextMenu?: Mutation<[PointerInteractEvent]>;
        behavior?: Val<HitTestBehavior>;
        queryKey?: Val<string[]>;
        child?: Element;
    }

    export interface MouseRegionProps {
        cursor?: Val<Cursor>;
        onEnter?: Mutation<[PointerRegionEvent]>;
        onExit?: Mutation<[PointerRegionEvent]>;
        behavior?: Val<HitTestBehavior>;
        child?: Element;
    }

    export interface ConditionProps {
        condition: Val<boolean>;
        child?: () => Element;
        elseChild?: () => Element;
        queryKey?: Val<string[]>;
    }

    export interface SwitchCase {
        key: string | number | boolean | null | undefined;
        child: () => Element;
    }

    export interface SwitchProps {
        value: Val<string | number | boolean | null | undefined>;
        cases: SwitchCase[];
        fallback?: () => Element;
        queryKey?: Val<string[]>;
    }

    export interface ScrollViewProps {
        axis?: Val<Axis>;
        padding?: Val<number>;
        color?: Val<Brush | null>;
        controller?: ScrollController;
        child: Element;
        queryKey?: Val<string[]>;
    }

    export interface ScrollbarProps {
        controller?: ScrollController;
        color?: Val<Brush | null>;
        trackColor?: Val<Brush | null>;
        thickness?: Val<number>;
        thumbRadius?: Val<number>;
        queryKey?: Val<string[]>;
    }

    export interface LazyListProps {
        axis?: Val<Axis>;
        itemCount: Val<number>;
        overscan?: Val<number>;
        itemExtent?: Val<number>;
        builder: (index: number) => Element;
        queryKey?: Val<string[]>;
    }

    /** A non-scrollable grid that tiles its static `children` row-major. The
     *  column count is derived from the available cross-axis size and
     *  `maxCrossAxisExtent` (`count = floor(width / maxCrossAxisExtent)`). Cell
     *  main-axis size is `mainAxisExtent` if given, else
     *  `cell_cross / childAspectRatio` (default square). */
    export interface GridProps {
        maxCrossAxisExtent: Val<number>;
        childAspectRatio?: Val<number>;
        mainAxisExtent?: Val<number>;
        crossAxisSpacing?: Val<number>;
        mainAxisSpacing?: Val<number>;
        children: Element[];
        queryKey?: Val<string[]>;
    }

    /** A scrollable, virtualized grid. Only the cells inside the viewport +
     *  overscan are mounted. Same sizing model as `Grid`. `builder` receives
     *  the flat item `index`; row/col are derived from `crossAxisCount`. */
    export interface LazyGridProps {
        axis?: Val<Axis>;
        itemCount: Val<number>;
        maxCrossAxisExtent: Val<number>;
        childAspectRatio?: Val<number>;
        mainAxisExtent?: Val<number>;
        crossAxisSpacing?: Val<number>;
        mainAxisSpacing?: Val<number>;
        overscan?: Val<number>;
        builder: (index: number) => Element;
        queryKey?: Val<string[]>;
    }

    export interface EachProps<T> {
        items: Readable<T[]>;
        build: (item: T, index: number) => Element;
        mainAlignment?: Val<MainAxisAlignment>;
        crossAlignment?: Val<CrossAxisAlignment>;
        mainAxisSize?: Val<MainAxisSize>;
        queryKey?: Val<string[]>;
    }

    export interface ImageProps {
        resourceId: Val<number>;
        width?: Val<number>;
        height?: Val<number>;
        fit?: Val<BoxFit>;
        queryKey?: Val<string[]>;
        child?: Element;
    }

    export interface InputProps {
        controller?: TextController;
        undoController?: UndoController;
        placeholder?: Val<string>;
        color?: Val<Brush | null>;
        placeholderColor?: Val<Brush | null>;
        cursorColor?: Val<Brush | null>;
        fontSize?: Val<number>;
        fontFamily?: Val<string>;
        /** CSS-style numeric font weight (100–1000). Omit for the default
         *  (400). Per-span `weight` overrides it. */
        fontWeight?: Val<number>;
        width?: Val<number>;
        height?: Val<number>;
        multiline?: Val<boolean>;
        /** When true, each character is rendered as `obscuringCharacter`
         *  (password mode). The controller's `text` keeps the real value. */
        obscureText?: Val<boolean>;
        /** Mask glyph used when `obscureText` is on (default `"•"`). */
        obscuringCharacter?: Val<string>;
        onContextMenu?: Mutation<[PointerInteractEvent]>;
        queryKey?: Val<string[]>;
    }

    export interface FragmentProps {
        children: Element[];
    }

    export interface FocusableProps {
        onKeyDown?: Mutation<[KeyEvent]>;
        onKeyUp?: Mutation<[KeyEvent]>;
        onFocus?: Mutation<[]>;
        onBlur?: Mutation<[]>;
        child?: Element;
    }

    export interface ReadableSubscribeProps {
        readables: Readable<unknown>[];
        onUpdate$: Mutation<[]>;
        child: Element;
    }

    export interface LifecycleDescriptor {
        element: Element;
        onMounted$?: Mutation<[]>;
        beforeDestroy$?: Mutation<[]>;
    }

    // ---------------------------------------------------------------------------
    // Controllers
    // ---------------------------------------------------------------------------

    export interface TextEditingControllerOpts {
        initialText?: string;
        onInput?: Mutation<[string, boolean], void>;
        onCursorChange?: Mutation<[number], void>;
        onSelectionChange?: Mutation<[number, number], void>;
        onKeyDown?: Mutation<[KeyEvent], void>;
        onKeyUp?: Mutation<[KeyEvent], void>;
        onFocus?: Mutation<[], void>;
        onBlur?: Mutation<[], void>;
        onCompositionStart?: Mutation<[], void>;
        onCompositionUpdate?: Mutation<[string], void>;
        onCompositionEnd?: Mutation<[string], void>;
    }

    /** Text-edit controller (registered boa class). Built via
     *  `createTextEditingController`. Exposes the editable buffer + selection. */
    export interface TextController {
        /** The full buffer text. */
        readonly text: string;
        /** Current cursor offset (byte index into `text`). */
        readonly cursorPosition: number;
        /** Selection anchor (start) byte offset. */
        readonly selectionAnchor: number;
        /** Selection end byte offset. */
        readonly selectionEnd: number;
        /** The currently selected text, or `""` if no selection. */
        readonly selectedText: string;
        /** Replace the rich-text span list. */
        setSpans(spans: SpanData[]): void;
        /** Replace spans without moving the cursor. */
        setSpansPreserveCursor(spans: SpanData[]): void;
        /** Clear all text and spans. */
        clear(): void;
        /** Set the selection range `[anchor, end)` (byte offsets). */
        setSelection(anchor: number, end: number): void;
        /** Replace the current selection with `text`, or insert at the cursor. */
        insertText(text: string): void;
        /** Delete the current selection, if any. */
        deleteSelection(): void;
        /** Attach an `UndoController` so edits record undo history. */
        setUndoController(undo: UndoController): void;
        /** Focus the bound input. */
        requestFocus(): void;
    }

    export interface UndoController {
        readonly canUndo: boolean;
        readonly canRedo: boolean;
        clear(): void;
    }

    export interface ScrollControllerOpts {
        onScroll?: Mutation<[ScrollEvent], void>;
        initialOffset?: number;
    }

    /** Scroll controller (registered boa class). Built via `createScrollController`.
     *  Pair with a `ScrollView` / `Scrollbar` via the `controller` prop. */
    export interface ScrollController {
        readonly offset: number;
        readonly maxScrollExtent: number;
        readonly viewportDimension: number;
        /** Jump to `offset` (clamped to the scroll bounds). */
        jumpTo(offset: number): void;
    }

    export interface LazyListControllerOpts {
        onScroll?: Mutation<[ScrollEvent], void>;
        onVisibleRangeChange?: Mutation<[number, number], void>;
    }

    /** Lazy-list controller (registered boa class). Built via
     *  `createLazyListController`. Pair with a `LazyList` via the `controller` prop
     *  (the prop is currently read implicitly — pass the same instance). */
    export interface LazyListController {
        readonly offset: number;
        readonly maxScrollExtent: number;
        readonly viewportDimension: number;
        jumpTo(offset: number): void;
    }

    export interface LazyGridControllerOpts {
        onScroll?: Mutation<[ScrollEvent], void>;
        onVisibleRangeChange?: Mutation<[number, number], void>;
    }

    /** Lazy-grid controller (registered boa class). Built via
     *  `createLazyGridController`. Mirrors `LazyListController`. */
    export interface LazyGridController {
        readonly offset: number;
        readonly maxScrollExtent: number;
        readonly viewportDimension: number;
        jumpTo(offset: number): void;
    }

    // ---------------------------------------------------------------------------
    // Async task primitives — `sleep` (a timer primitive) + `launch` (a
    // cancellable generator coroutine driver). These replace the old
    // `setTimeout` / `setInterval` globals.
    // ---------------------------------------------------------------------------

    /** Resolve after `ms` milliseconds (engine time). The engine's frame loop
     *  wakes precisely at the deadline. Use bare (`sleep(ms).then(...)`) or
     *  inside a `launch` coroutine via `yield sleep(ms)`. */
    export function sleep(ms: number): Promise<void>;

    /** A cancellable coroutine task returned by `launch`. `cancel()` stops the
     *  generator from resuming after its current `yield`. Any in-flight
     *  `sleep` resolves harmlessly and is ignored. */
    export interface Task {
        cancel(): void;
    }

    /** Run a zero-arg generator function as a cancellable coroutine. The
     *  generator must `yield` Promises (typically `sleep(ms)`); each resolved
     *  promise resumes the generator, passing the resolved value back as the
     *  `yield` result. Returns a `Task` whose `cancel()` halts further
     *  resumption.
     *
     *  Rejections: when a yielded promise rejects, the rejection reason is
     *  thrown into the generator at the `yield` point — so a `try/catch`
     *  around `yield` catches it (the same ergonomics as `await`). An uncaught
     *  rejection stops the coroutine. This makes `launch` safe to use with
     *  fallible Promises (`clipboard.readText`, `http`, `fetch`), not just
     *  `sleep`.
     *
     *  Unlike `async`/`await`, generators can be externally stepped/abandoned,
     *  which is what makes real cancellation possible. Use the debounce
     *  pattern: `task?.cancel(); task = launch(function* () { yield sleep(ms);
     *  ... });`. */
    export function launch<T>(
        gen: () => Generator<Promise<unknown>, T, unknown>,
    ): Task;

    // ---------------------------------------------------------------------------
    // Element factories
    // ---------------------------------------------------------------------------

    export function Container(props: ContainerProps): Element;

    /** A width/height-only `Container` (no decoration, no child layout props beyond
     *  `children`). Sugar for `Container({ width, height, children })`. */
    export function SizedBox(props: {
        width?: Val<number>;
        height?: Val<number>;
        children?: Element[];
    }): Element;
    export function Column(props: FlexProps): Element;
    export function Row(props: FlexProps): Element;
    export function Expanded(props: ExpandedProps): Element;
    export function Stack(props: StackProps): Element;
    export function Positioned(props: PositionedProps): Element;
    export function Text(props: TextProps): Element;
    export function PointerInteract(props: PointerInteractProps): Element;
    export function MouseRegion(props: MouseRegionProps): Element;
    export function Condition(props: ConditionProps): Element;
    export function Switch(props: SwitchProps): Element;
    export function Each<T>(props: EachProps<T>): Element;
    export function LazyList(props: LazyListProps): Element;
    export function Grid(props: GridProps): Element;
    export function LazyGrid(props: LazyGridProps): Element;
    export function ScrollView(props: ScrollViewProps): Element;
    export function Scrollbar(props: ScrollbarProps): Element;
    export function Image(props: ImageProps): Element;
    export function Input(props: InputProps): Element;
    export function Fragment(props: FragmentProps): Element;
    export function Focusable(props: FocusableProps): Element;
    export function lifecycleView(f: () => LifecycleDescriptor): Element;
    export function ReadableSubscribe(props: ReadableSubscribeProps): Element;

    // ---------------------------------------------------------------------------
    // Visual-effect elements (Opacity / Transform)
    // ---------------------------------------------------------------------------

    export interface OpacityProps {
        value: Val<number>;
        child?: Element;
        queryKey?: Val<string[]>;
    }

    export interface TransformProps {
        scale?: Val<number>;
        scaleX?: Val<number>;
        scaleY?: Val<number>;
        rotate?: Val<number>;
        translateX?: Val<number>;
        translateY?: Val<number>;
        /** Pivot for `rotate`/`scale`, within the child box. Defaults to
         *  `Alignment.Center` (matches Flutter's `Transform`). */
        alignment?: Val<Alignment>;
        child?: Element;
        queryKey?: Val<string[]>;
    }

    /** Alpha-mask its child subtree by `value` (0.0..=1.0). */
    export function Opacity(props: OpacityProps): Element;

    /** Apply a 2D affine rotate/scale/translate to its child subtree. */
    export function Transform(props: TransformProps): Element;

    // ---------------------------------------------------------------------------
    // CompositedTransformTarget / Follower — Flutter-style anchor linking.
    // A follower renders at a target's global position (tracked continuously
    // through layout / scroll / reactive / transform changes). Create a shared
    // link via `createLayerLink()` and pass it to one target + one follower.
    // Place the follower in a root overlay slot so it isn't clipped and paints
    // on top (the Flutter `Overlay` pattern).
    // ---------------------------------------------------------------------------

    /** Shared handle connecting one `CompositedTransformTarget` to one
     *  `CompositedTransformFollower`. Create via `createLayerLink()`. */
    export interface LayerLink {
        readonly _layerLinkBrand: unique symbol;
    }

    /** Create a shared `LayerLink` connecting a target and a follower. */
    export function createLayerLink(): LayerLink;

    export interface CompositedTransformTargetProps {
        /** A link created via `createLayerLink()`, shared with the follower. */
        link: LayerLink;
        child?: Element;
        queryKey?: Val<string[]>;
    }

    export interface CompositedTransformFollowerProps {
        /** A link created via `createLayerLink()`, shared with the target. */
        link: LayerLink;
        /** Anchor point on the target that the follower aligns to. The
         *  follower is translated so its `followerAnchor` lands here in global
         *  space. Defaults to `Alignment.TopLeft`. Reactive: pass a `derive`
         *  to change it at runtime. */
        targetAnchor?: Val<Alignment>;
        /** Anchor point on this follower that lines up with `targetAnchor`.
         *  Defaults to `Alignment.TopLeft`. Reactive. */
        followerAnchor?: Val<Alignment>;
        /** Additional offset (in the target's local coordinate space) applied
         *  to `targetAnchor`. Defaults to `{x: 0, y: 0}`. Reactive: pass a
         *  `derive` to change it at runtime (e.g. steppers). */
        targetOffset?: Val<{ x: number; y: number }>;
        /** Whether to keep rendering at the follower's layout position when no
         *  target is linked. Defaults to `true`. */
        showWhenUnlinked?: boolean;
        child?: Element;
        queryKey?: Val<string[]>;
    }

    /** Marks a spot in the tree for a `CompositedTransformFollower` to track.
     *  A transparent passthrough. */
    export function CompositedTransformTarget(
        props: CompositedTransformTargetProps,
    ): Element;

    /** Renders at a target's anchor (tracked continuously). Place in a root
     *  overlay slot. */
    export function CompositedTransformFollower(
        props: CompositedTransformFollowerProps,
    ): Element;

    // ---------------------------------------------------------------------------
    // Controllers / resources / colors / focus
    // ---------------------------------------------------------------------------

    export function createTextEditingController(
        opts?: TextEditingControllerOpts,
    ): TextController;
    export function createUndoController(): UndoController;
    export function createScrollController(
        opts?: ScrollControllerOpts,
    ): ScrollController;
    export function createLazyListController(
        opts?: LazyListControllerOpts,
    ): LazyListController;
    export function createLazyGridController(
        opts?: LazyGridControllerOpts,
    ): LazyGridController;
    export function createImageResource(
        bytes: Uint8Array | ArrayBuffer,
    ): number;
    export function createSvgResource(svg: string): number;
    export function createColor(
        r: number,
        g: number,
        b: number,
        a: number,
    ): Color;
    export function createLinearGradient(
        sx: number,
        sy: number,
        ex: number,
        ey: number,
        stops: Array<{
            offset: number;
            r: number;
            g: number;
            b: number;
            a: number;
        }>,
    ): LinearGradient;
    export function colorLerp(a: Color, b: Color, t: number): Color;
    export function requestFocus(target: TextController | Element): void;

    export interface EventBus {
        on(channelId: number, callback: (payload: Uint8Array) => void): void;
        send(channelId: number, payload: Uint8Array): void;
    }
    export const eventBus: EventBus;

    /** Decode a Uint8Array (or ArrayBuffer) of UTF-8 bytes into a string. */
    export function decodeUtf8(bytes: Uint8Array | ArrayBuffer): string;

    /** Encode a string into a Uint8Array of UTF-8 bytes. */
    export function encodeUtf8(text: string): Uint8Array;
}
