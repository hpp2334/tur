/**
 * @tur-ng/std — ambient type declarations for the native tur widget library.
 *
 * Runtime is a synthetic boa module registered by tur-engine under the
 * specifier `"tur:std"`. It re-exports everything from
 * `"tur:core"` (the reactive primitives + meta-types) and adds the
 * widget layer: element builders, enums, value types (Color /
 * LinearGradient / SpanData), view controllers, resources, and the event
 * detail payloads.
 *
 * Consumers typically import from `tur:std` alone — it is the
 * convenience superset:
 * ```ts
 * import { Container, Column, source, Color, Axis } from "tur:std";
 * ```
 *
 * ## Builder pattern
 *
 * Every element constructor takes a small props object with its
 * **required** props only (or nothing when there are none) and returns a
 * chainable builder. Each prop is also a builder method (camelCase,
 * identical to the historical prop key; later calls overwrite), `.children`
 * appends, `.child` sets the single child, and `.build()` materializes the
 * `Element`:
 * ```ts
 * Container()
 *     .padding(16)
 *     .color(bg)
 *     .children([Text({ text: "hi" }).fontSize(14).build()])
 *     .build();
 * ```
 *
 * `@tur-ng/animation` and other libraries that need only the reactive
 * substrate may import directly from `tur:core`.
 */

/// <reference types="@tur-ng/core" />

declare module "tur:std" {
    // Re-export the reactive core (source/derive/mutate/get/set/view/mount,
    // Element/Source/Derived/Mutation/Readable/Val, ReadonlyStoreCtx/StoreCtx).
    export * from "tur:core";

    // Core meta-types used by the builder interfaces below. (`export *`
    // alone re-exports but does not bind names locally — every core type
    // used in this module body must also appear here.)
    import type { Derived, Element, Mutation, Readable, Val } from "tur:core";

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
     *  (`{width, height}` in CSS pixels). Published by the engine on every
     *  resize (readable through any store of the instance); import from
     *  `tur:std`. Read-only to app code — typed as a `Derived` so
     *  `set(viewportSize$, …)` is rejected at compile time. */
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

    export enum StackFit {
        Loose = 0,
        Expand = 1,
        Passthrough = 2,
    }

    // ---------------------------------------------------------------------------
    // Element builders — every element constructor takes a small required-props
    // object (or nothing) and returns a fluent builder terminated by `.build()`.
    // Prop methods accept the same `Val<…>` shapes the historical props object
    // did; later calls overwrite; `.children(arr)` appends; `.child(el)` sets.
    // ---------------------------------------------------------------------------

    /** Terminal of every element builder: materialize the `Element`. */
    export interface BuilderBuild {
        build(): Element;
    }

    export interface ContainerBuilder extends BuilderBuild {
        width(v: Val<number | undefined>): this;
        height(v: Val<number | undefined>): this;
        padding(v: Val<number | undefined>): this;
        color(v: Val<Brush | null | undefined>): this;
        borderColor(v: Val<Brush | null | undefined>): this;
        borderWidth(v: Val<number | undefined>): this;
        borderRadius(v: Val<number | undefined>): this;
        borderPosition(v: Val<BorderPosition | undefined>): this;
        clipBehavior(v: Val<ClipBehavior | undefined>): this;
        shadowColor(v: Val<Brush | null | undefined>): this;
        shadowOffset(v: Val<[number, number]> | undefined): this;
        shadowBlur(v: Val<number | undefined>): this;
        alignment(v: Val<Alignment | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        children(children: Element[]): this;
    }

    export interface FlexBuilder extends BuilderBuild {
        mainAlignment(v: Val<MainAxisAlignment | undefined>): this;
        crossAlignment(v: Val<CrossAxisAlignment | undefined>): this;
        mainAxisSize(v: Val<MainAxisSize | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        children(children: Element[]): this;
    }

    export interface ExpandedBuilder extends BuilderBuild {
        flex(v: Val<number | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        child(child: Element): this;
    }

    export interface StackBuilder extends BuilderBuild {
        /** How to size non-positioned children (default `StackFit.Loose`).
         *  `Expand` tightens them to the Stack's constraints — the idiomatic
         *  way to build a full-bleed background layer for overlay stacks. */
        fit(v: Val<StackFit | undefined>): this;
        /** Where to place non-positioned children (default TopLeft). */
        alignment(v: Val<Alignment | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        children(children: Element[]): this;
    }

    export interface PositionedBuilder extends BuilderBuild {
        left(v: Val<number | undefined>): this;
        top(v: Val<number | undefined>): this;
        right(v: Val<number | undefined>): this;
        bottom(v: Val<number | undefined>): this;
        width(v: Val<number | undefined>): this;
        height(v: Val<number | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        child(child: Element): this;
    }

    /** A non-scrollable grid that tiles its static `children` row-major. The
     *  column count is derived from the available cross-axis size and
     *  `maxCrossAxisExtent` (`count = floor(width / maxCrossAxisExtent)`). Cell
     *  main-axis size is `mainAxisExtent` if given, else
     *  `cell_cross / childAspectRatio` (default square). */
    export interface GridBuilder extends BuilderBuild {
        maxCrossAxisExtent(v: Val<number | undefined>): this;
        childAspectRatio(v: Val<number | undefined>): this;
        mainAxisExtent(v: Val<number | undefined>): this;
        crossAxisSpacing(v: Val<number | undefined>): this;
        mainAxisSpacing(v: Val<number | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        children(children: Element[]): this;
    }

    /** One `Table.columns` entry. `width` = fixed px; `flex` = share of the
     *  leftover width (proportional to the total flex weight). A column with
     *  neither defaults to `flex: 1`. `minWidth` clamps the distributed
     *  share up (may overflow the available width). */
    export interface TableColumnDef {
        width?: number;
        flex?: number;
        minWidth?: number;
    }

    /** A non-scrollable data table with shared column geometry: the header
     *  row and every body row lay their cells at the same resolved column
     *  widths (CSS `table-layout: fixed` semantics). `rows` is a reactive
     *  array — writing a new array value rebuilds the row subtrees (write a
     *  fresh array; in-place mutation of the same array object with an
     *  unchanged length is not observed). `rowBuilder` returns one row's
     *  cells, positionally mapped to the columns: a `null` entry is an empty
     *  cell box (the column advances), entries beyond the column count are
     *  ignored. `headerBuilder` follows the same mapping and runs ONCE at
     *  build — reactive header content flows through `Val` props inside
     *  the returned cells. Without an extent, a row's height is the max
     *  intrinsic cell height (cells get loose height constraints); with
     *  one, cells fill it. Wrap in a `ScrollView` to scroll. */
    export interface TableBuilder<T> extends BuilderBuild {
        columns(cols: TableColumnDef[]): this;
        rows(rows: Readable<T[]>): this;
        rowBuilder(fn: (item: T, index: number) => (Element | null)[]): this;
        headerBuilder(fn: () => (Element | null)[]): this;
        headerExtent(v: Val<number | undefined>): this;
        rowExtent(v: Val<number | undefined>): this;
        rowSpacing(v: Val<number | undefined>): this;
        /** Painted under odd body rows. */
        stripeColor(v: Val<Brush | null | undefined>): this;
        /** Horizontal rules between body rows + under the header. */
        dividerColor(v: Val<Brush | null | undefined>): this;
        dividerThickness(v: Val<number | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
    }

    /**
     * How `Text` handles content beyond `maxLines`. Mirrors Flutter's
     * `TextOverflow`.
     */
    export type TextOverflow = "clip" | "ellipsis" | "visible";

    export interface TextBuilder extends BuilderBuild {
        text(v: Val<string | undefined>): this;
        fontSize(v: Val<number | undefined>): this;
        /** CSS-style numeric font weight (100–1000) applied to the whole
         *  element. Per-span `weight` overrides it for that range. Omit for
         *  the default (400). */
        fontWeight(v: Val<number | undefined>): this;
        color(v: Val<Brush | null | undefined>): this;
        spans(v: SpanData[] | undefined): this;
        /** When `true`, the text can be drag-selected with the pointer. */
        selectable(v: boolean | undefined): this;
        onSelectionChange(m: Mutation<[unknown]> | undefined): this;
        queryKey(keys: Val<string[] | undefined>): this;
        /**
         * Maximum number of lines to render. Ignored when `overflow` is
         * `"visible"`. When omitted (or `0`), the text wraps without limit.
         */
        maxLines(v: Val<number | undefined>): this;
        /**
         * How content beyond `maxLines` is handled. Defaults to `"clip"`
         * when `maxLines` is set.
         * - `"clip"`     — render at most `maxLines` lines, discard the rest.
         * - `"ellipsis"` — render at most `maxLines` lines, appending `…`
         *                  to the last visible line (trimmed to fit).
         * - `"visible"`  — render all lines; `maxLines` is ignored.
         */
        overflow(v: Val<TextOverflow | undefined>): this;
    }

    export interface InputBuilder extends BuilderBuild {
        controller(
            c: TextController | Readable<TextController> | undefined,
        ): this;
        undoController(c: UndoController | undefined): this;
        placeholder(v: Val<string | undefined>): this;
        color(v: Val<Brush | null | undefined>): this;
        placeholderColor(v: Val<Brush | null | undefined>): this;
        cursorColor(v: Val<Brush | null | undefined>): this;
        fontSize(v: Val<number | undefined>): this;
        fontFamily(v: Val<string | undefined>): this;
        /** CSS-style numeric font weight (100–1000). Omit for the default
         *  (400). Per-span `weight` overrides it. */
        fontWeight(v: Val<number | undefined>): this;
        width(v: Val<number | undefined>): this;
        height(v: Val<number | undefined>): this;
        multiline(v: Val<boolean | undefined>): this;
        /** When true, each character is rendered as `obscuringCharacter`
         *  (password mode). The controller's `text` keeps the real value. */
        obscureText(v: Val<boolean | undefined>): this;
        /** Mask glyph used when `obscureText` is on (default `"•"`). */
        obscuringCharacter(v: Val<string | undefined>): this;
        onContextMenu(m: Mutation<[PointerInteractEvent]> | undefined): this;
        queryKey(keys: Val<string[] | undefined>): this;
    }

    export interface ImageBuilder extends BuilderBuild {
        resourceId(v: Val<number | undefined>): this;
        width(v: Val<number | undefined>): this;
        height(v: Val<number | undefined>): this;
        fit(v: Val<BoxFit | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        child(child: Element): this;
    }

    export interface ScrollViewBuilder extends BuilderBuild {
        axis(v: Val<Axis | undefined>): this;
        padding(v: Val<number | undefined>): this;
        color(v: Val<Brush | null | undefined>): this;
        controller(c: ScrollController | undefined): this;
        queryKey(keys: Val<string[] | undefined>): this;
        child(child: Element): this;
    }

    export interface ScrollbarBuilder extends BuilderBuild {
        color(v: Val<Brush | null | undefined>): this;
        trackColor(v: Val<Brush | null | undefined>): this;
        thickness(v: Val<number | undefined>): this;
        thumbRadius(v: Val<number | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
    }

    export interface LazyListBuilder extends BuilderBuild {
        itemCount(v: Val<number | undefined>): this;
        builder(fn: (index: number) => Element): this;
        axis(v: Val<Axis | undefined>): this;
        overscan(v: Val<number | undefined>): this;
        itemExtent(v: Val<number | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
    }

    /** A scrollable, virtualized grid. Only the cells inside the viewport +
     *  overscan are mounted. Same sizing model as `Grid`. `builder` receives
     *  the flat item `index`; row/col are derived from `crossAxisCount`. */
    export interface LazyGridBuilder extends BuilderBuild {
        itemCount(v: Val<number | undefined>): this;
        maxCrossAxisExtent(v: Val<number | undefined>): this;
        builder(fn: (index: number) => Element): this;
        axis(v: Val<Axis | undefined>): this;
        overscan(v: Val<number | undefined>): this;
        childAspectRatio(v: Val<number | undefined>): this;
        mainAxisExtent(v: Val<number | undefined>): this;
        crossAxisSpacing(v: Val<number | undefined>): this;
        mainAxisSpacing(v: Val<number | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
    }

    export interface OpacityBuilder extends BuilderBuild {
        value(v: Val<number | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        child(child: Element): this;
    }

    export interface TransformBuilder extends BuilderBuild {
        scale(v: Val<number | undefined>): this;
        scaleX(v: Val<number | undefined>): this;
        scaleY(v: Val<number | undefined>): this;
        rotate(v: Val<number | undefined>): this;
        translateX(v: Val<number | undefined>): this;
        translateY(v: Val<number | undefined>): this;
        /** Pivot for `rotate`/`scale`, within the child box. Defaults to
         *  `Alignment.Center` (matches Flutter's `Transform`). */
        alignment(v: Val<Alignment | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        child(child: Element): this;
    }

    export interface CompositedTransformTargetBuilder extends BuilderBuild {
        /** A link created via `createLayerLink()`, shared with the follower. */
        link(link: LayerLink): this;
        child(child: Element): this;
    }

    export interface CompositedTransformFollowerBuilder extends BuilderBuild {
        /** A link created via `createLayerLink()`, shared with the target. */
        link(link: LayerLink): this;
        /** Anchor point on the target that the follower aligns to. The
         *  follower is translated so its `followerAnchor` lands here in global
         *  space. Defaults to `Alignment.TopLeft`. Reactive: pass a `derive`
         *  to change it at runtime. */
        targetAnchor(v: Val<Alignment | undefined>): this;
        /** Anchor point on this follower that lines up with `targetAnchor`.
         *  Defaults to `Alignment.TopLeft`. Reactive. */
        followerAnchor(v: Val<Alignment | undefined>): this;
        /** Additional offset (in the target's local coordinate space) applied
         *  to `targetAnchor`. Defaults to `{x: 0, y: 0}`. Reactive: pass a
         *  `derive` to change it at runtime (e.g. steppers). */
        targetOffset(v: Val<{ x: number; y: number }> | undefined): this;
        /** Whether to keep rendering at the follower's layout position when no
         *  target is linked. Defaults to `true`. */
        showWhenUnlinked(v: boolean | undefined): this;
        child(child: Element): this;
    }

    export interface ConditionBuilder extends BuilderBuild {
        condition(v: Val<boolean | undefined>): this;
        /** The branch thunk (`() => Element`), re-invoked on condition flips. */
        child(fn: () => Element): this;
        elseChild(fn: (() => Element) | undefined): this;
        queryKey(keys: Val<string[] | undefined>): this;
    }

    export interface SwitchCase {
        key: string | number | boolean | null | undefined;
        child: () => Element;
    }

    export interface SwitchBuilder extends BuilderBuild {
        value(
            v: Val<string | number | boolean | null | undefined | undefined>,
        ): this;
        cases(cases: SwitchCase[] | undefined): this;
        fallback(fn: (() => Element) | undefined): this;
        queryKey(keys: Val<string[] | undefined>): this;
    }

    export interface EachBuilder<T> extends BuilderBuild {
        items(items: Readable<T[]>): this;
        /** The item factory — the historical `build` prop (renamed: `build`
         *  is the builder's terminal). */
        itemBuilder(fn: (item: T, index: number) => Element): this;
        queryKey(keys: Val<string[] | undefined>): this;
    }

    export interface FragmentBuilder extends BuilderBuild {
        queryKey(keys: Val<string[] | undefined>): this;
        children(children: Element[]): this;
    }

    export interface PointerInteractBuilder extends BuilderBuild {
        onClick(m: Mutation<[PointerInteractEvent]> | undefined): this;
        onPointerDown(m: Mutation<[PointerInteractEvent]> | undefined): this;
        onPointerMove(m: Mutation<[PointerInteractEvent]> | undefined): this;
        onPointerUp(m: Mutation<[PointerInteractEvent]> | undefined): this;
        onContextMenu(m: Mutation<[PointerInteractEvent]> | undefined): this;
        behavior(v: Val<HitTestBehavior | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        child(child: Element): this;
    }

    export interface MouseRegionBuilder extends BuilderBuild {
        cursor(v: Val<Cursor | undefined>): this;
        onEnter(m: Mutation<[PointerRegionEvent]> | undefined): this;
        onExit(m: Mutation<[PointerRegionEvent]> | undefined): this;
        behavior(v: Val<HitTestBehavior | undefined>): this;
        queryKey(keys: Val<string[] | undefined>): this;
        child(child: Element): this;
    }

    export interface FocusableBuilder extends BuilderBuild {
        onKeyDown(m: Mutation<[KeyEvent]> | undefined): this;
        onKeyUp(m: Mutation<[KeyEvent]> | undefined): this;
        onFocus(m: Mutation<[]> | undefined): this;
        onBlur(m: Mutation<[]> | undefined): this;
        child(child: Element): this;
    }

    export interface ReadableSubscribeBuilder extends BuilderBuild {
        readables(readables: Readable<unknown>[]): this;
        onUpdate$(m: Mutation<[]> | undefined): this;
        child(child: Element): this;
    }

    export interface VirtualAppViewBuilder extends BuilderBuild {
        /** Reactive controller binding — `null` unbinds (destroys). */
        app$(app: Readable<VirtualAppController | null>): this;
        background(v: Val<Color | undefined>): this;
        width(v: Val<number | undefined>): this;
        height(v: Val<number | undefined>): this;
        queryKey(keys: string[] | undefined): this;
        /** Painted while the child isn't live. */
        fallback(child: Element | undefined): this;
        /** Painted on error. */
        errorView(child: Element | undefined): this;
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
    // Async task primitives — the `Task<T>` handle, `sleep`, `CancelError`,
    // and `isCancelError`. These replace the old `setTimeout` /
    // `setInterval` globals (and the former `launch` generator driver):
    // async composition is plain `async`/`await` + `.then`, cancellation is
    // per-operation via the Task handle.
    // ---------------------------------------------------------------------------

    /** The handle every async engine API returns: `sleep`, `request`,
     *  `requestStream`, `clipboard.readText`/`writeText`,
     *  `filePicker.pick`/`saveFile`, …
     *
     *  - `promise` settles with the operation's result.
     *  - `cancel()` stops the operation where stoppable (a pending `sleep`
     *    timer is really cleared; an unpolled HTTP request is never sent;
     *    an in-flight one is discarded; a stream is wire-aborted) and
     *    **rejects `promise` with a `CancelError`**. Idempotent; a no-op
     *    for the promise once settled (op-specific abort still runs — e.g.
     *    cancelling a stream mid-consumption).
     *
     *  Debounce idiom (the no-op rejection handler IS the cancelled
     *  branch):
     *  ```ts
     *  t?.cancel(); t = sleep(300);
     *  t.promise.then(show, () => {});
     *  ```
     *
     *  Loop stop idiom: cancel the awaited sleep; the `await` throws
     *  `CancelError`; `catch (e) { if (isCancelError(e)) return; throw e; }`
     *  exits the loop. */
    export interface Task<T> {
        readonly promise: Promise<T>;
        cancel(): void;
    }

    /** The rejection reason produced by `Task.cancel()` — an `Error` whose
     *  `name` is `"CancelError"`. Test with `isCancelError` (or
     *  `e.name === "CancelError"`). */
    export interface CancelError extends Error {
        name: "CancelError";
    }

    /** `true` when `reason` is the rejection produced by `Task.cancel()`. */
    export function isCancelError(reason: unknown): reason is CancelError;

    /** Sleep for `ms` milliseconds (engine time) — the engine's frame loop
     *  wakes precisely at the deadline. Returns a `Task<void>`: await
     *  `sleep(ms).promise`, and `cancel()` to clear the timer (the promise
     *  then rejects with a `CancelError`). */
    export function sleep(ms: number): Task<void>;

    // ---------------------------------------------------------------------------
    // Element factories — each takes its required props (or nothing) and
    // returns a chainable builder terminated by `.build()`.
    // ---------------------------------------------------------------------------

    export function Container(): ContainerBuilder;
    export function SizedBox(props?: {
        width?: Val<number>;
        height?: Val<number>;
    }): ContainerBuilder;
    export function Column(): FlexBuilder;
    export function Row(): FlexBuilder;
    export function Expanded(): ExpandedBuilder;
    export function Stack(): StackBuilder;
    export function Positioned(): PositionedBuilder;
    export function Text(props?: { text?: Val<string> }): TextBuilder;
    export function PointerInteract(): PointerInteractBuilder;
    export function MouseRegion(): MouseRegionBuilder;
    export function Condition(props?: {
        condition?: Val<boolean>;
    }): ConditionBuilder;
    export function Switch(props?: {
        value?: Val<string | number | boolean | null | undefined>;
    }): SwitchBuilder;
    export function Each<T>(props: { items: Readable<T[]> }): EachBuilder<T>;
    export function LazyList(props: {
        itemCount: Val<number>;
    }): LazyListBuilder;
    export function Grid(props: {
        maxCrossAxisExtent: Val<number>;
    }): GridBuilder;
    export function Table<T>(props: {
        columns: TableColumnDef[];
        rows: Readable<T[]>;
    }): TableBuilder<T>;
    export function LazyGrid(props: {
        itemCount: Val<number>;
        maxCrossAxisExtent: Val<number>;
    }): LazyGridBuilder;
    export function ScrollView(): ScrollViewBuilder;
    export function Scrollbar(): ScrollbarBuilder;
    export function Image(props?: { resourceId?: Val<number> }): ImageBuilder;
    export function Input(): InputBuilder;
    export function Fragment(): FragmentBuilder;
    export function Focusable(): FocusableBuilder;
    export function lifecycleView(f: () => LifecycleDescriptor): Element;
    export function ReadableSubscribe(): ReadableSubscribeBuilder;

    // ---------------------------------------------------------------------------
    // Visual-effect elements (Opacity / Transform)
    // ---------------------------------------------------------------------------

    /** Alpha-mask its child subtree by `value` (0.0..=1.0). */
    export function Opacity(props?: { value?: Val<number> }): OpacityBuilder;

    /** Apply a 2D affine rotate/scale/translate to its child subtree. */
    export function Transform(): TransformBuilder;

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

    /** Marks a spot in the tree for a `CompositedTransformFollower` to track.
     *  A transparent passthrough. */
    export function CompositedTransformTarget(props: {
        link: LayerLink;
    }): CompositedTransformTargetBuilder;

    /** Renders at a target's anchor (tracked continuously). Place in a root
     *  overlay slot. */
    export function CompositedTransformFollower(props: {
        link: LayerLink;
    }): CompositedTransformFollowerBuilder;

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

    // ------------------------------------------------------------------
    // Virtual apps — VirtualAppView hosts a complete nested engine
    // instance (own worker, realm, store, tree) and draws the child's
    // frames from its own paint. The controller is a lazy declaration:
    // nothing runs until an element binds it (`app$` resolving to a
    // controller); unbinding destroys the child unless `keepAlive`.
    // ------------------------------------------------------------------

    /**
     * Opaque handle to a registered module source (`createModuleSource`).
     * The source string never crosses the JS API again — only this handle
     * does (the JS mirror of the Rust-side `ModuleSourceRegistry` /
     * `load_module_source` flow).
     */
    export interface ModuleSourceHandle {
        readonly __moduleSource: unique symbol;
    }

    /**
     * Opaque handle to a worker pool registered on the runtime
     * (`forWorkerPool`). The JS mirror of the Rust-side `WorkerPoolHandle`
     * — resolved eagerly against the registry, so an unknown name throws
     * at the call site.
     */
    export interface WorkerPoolHandle {
        readonly __workerPool: unique symbol;
    }

    /** Lifecycle state of a hosted virtual app. */
    export type VirtualAppStatus =
        | "idle"
        | "spawning"
        | "running"
        | "error"
        | "destroyed";

    /** A hosted virtual app's controller — a lazy declaration. */
    export interface VirtualAppController {
        /** Reactive status — read via `store.get(app.status$)`. */
        readonly status$: Readable<VirtualAppStatus>;
        /** Error detail (module load / start failures) — read via `store.get`. */
        readonly errorMsg$: Readable<string>;
        /**
         * The ONLY lifecycle action — a control mutation (the `watch`
         * `{ start$, stop$ }` convention: side effects ride the mutation
         * rail). Dispatch via `store.set(app.destroy$)`. New code =
         * destroy$ + a new controller with a new source.
         */
        readonly destroy$: Mutation;
    }

    /** Register a module source once; reference it by opaque handle. */
    export function createModuleSource(source: string): ModuleSourceHandle;

    /**
     * Resolve a registered worker pool by name — eagerly (an unknown name
     * throws right here). The only source of the handle
     * `createVirtualAppController({ pool })` accepts.
     */
    export function forWorkerPool(name: string): WorkerPoolHandle;

    /** Create a virtual-app controller (lazy — spawns on first bind). */
    export function createVirtualAppController(opts: {
        source: ModuleSourceHandle;
        /**
         * Target worker pool, from `forWorkerPool(name)` (handle-only — raw
         * strings are rejected). Omit for the default `"virtual"` pool.
         */
        pool?: WorkerPoolHandle;
        /** Survive element unbind (default `false`). */
        keepAlive?: boolean;
        /**
         * Notified of runtime JS errors inside the child after it reached
         * `"running"`: throws from mutations / view closures / factories /
         * microtask + async callbacks, and promise rejections that still
         * have no handler at the end of a frame (a same-frame `.catch`
         * retracts). The callback receives a reconstructed `Error` minted in
         * the parent realm (`e instanceof Error`, `e.message`, best-effort
         * `e.stack`) — the child's thrown value never crosses the worker
         * boundary.
         *
         * Notification only: does NOT fire for module load/start failures
         * (those ride `status$` / `errorMsg$` / `errorView`) and does not
         * change `status$`. Coalesced: at most one dispatch per frame.
         *
         * ```js
         * const app = createVirtualAppController({
         *     source,
         *     onRuntimeError$: mutate((ctx, e) => log(e.message, e.stack)),
         * });
         * ```
         */
        onRuntimeError$?: Mutation<[unknown]>;
    }): VirtualAppController;

    /** Element hosting a virtual app; draws the child's latest frame. */
    export function VirtualAppView(props?: {
        /** Reactive controller binding — `null` unbinds (destroys). */
        app$?: Readable<VirtualAppController | null>;
    }): VirtualAppViewBuilder;
}
