/**
 * tur-edgy — Reactive widget composition layer for tur.
 *
 * Replaces the old React-based API (@tur/react-renderer + @tur/react). All
 * reactivity lives in the Rust store (signals/derive/mutate); JS code just
 * composes built-in widget factories and supplies closures for derive/mutate
 * callbacks.
 */

// ---------------------------------------------------------------------------
// Reactive primitives — thin wrappers over `__tur.*`. Atoms are opaque
// handles; closures receive a `{get, set}` ctx as their first argument.
// ---------------------------------------------------------------------------

export interface Atom<T> {
    readonly __turAtom?: T;
}
export interface Mutation<Args extends unknown[] = [], R = void> {
    readonly __turMutation?: [Args, R];
}
export type Readable<T> = Atom<T>;
export type Val<T> = T | Readable<T>;

declare const __tur: TurGlobal;

/** The bridge context (opaque handle to `TurJsContext` in Rust). Captured
 * once at module-load time; bridge fns take it as their first arg. */
const __ctx: unknown = (__tur as unknown as { __ctx: unknown }).__ctx;

export interface StoreCtx {
    get<T>(a: Readable<T>): T;
    set<T>(s: Atom<T>, value: T): void;
    set<Args extends unknown[], R>(m: Mutation<Args, R>, ...args: Args): R;
}

export function source<T>(value: T): Atom<T> {
    return __tur.source(__ctx, value) as Atom<T>;
}

export function derive<T>(fn: (get: StoreCtx["get"]) => T): Readable<T> {
    return __tur.derive(
        __ctx,
        (ctx: StoreCtx) => fn(ctx.get.bind(ctx)) as T,
    ) as Readable<T>;
}

export function mutate<Args extends unknown[], R>(
    fn: (ctx: StoreCtx, ...args: Args) => R,
): Mutation<Args, R> {
    return __tur.mutate(
        __ctx,
        fn as (ctx: StoreCtx, ...args: unknown[]) => unknown,
    ) as Mutation<Args, R>;
}

export function get<T>(a: Readable<T>): T {
    return __tur.get(__ctx, a) as T;
}

export function set<T>(s: Atom<T>, value: T): void;
export function set<Args extends unknown[], R>(
    m: Mutation<Args, R>,
    ...args: Args
): R;
export function set(target: unknown, ...rest: unknown[]): unknown {
    return __tur.set(__ctx, target, ...rest);
}

export function isReadable(x: unknown): x is Readable<unknown> {
    return typeof x === "object" && x !== null;
}

// ---------------------------------------------------------------------------
// Component helper — wraps a `() => EdgyElement` thunk as a component handle.
// The thunk is invoked lazily by the engine when the component is built.
// ---------------------------------------------------------------------------

export type EdgyComponent = EdgyElement;

export function component(f: () => EdgyElement): EdgyComponent {
    return __tur.component(__ctx, f) as EdgyComponent;
}

// ---------------------------------------------------------------------------
// EdgyElement handle — opaque reference returned by widget factories.
// ---------------------------------------------------------------------------

export type EdgyElement = unknown;

// ---------------------------------------------------------------------------
// Widget factories.
// ---------------------------------------------------------------------------

export interface ContainerProps {
    width?: Val<number>;
    height?: Val<number>;
    padding?: Val<number>;
    color?: Val<unknown>;
    borderColor?: Val<unknown>;
    borderWidth?: Val<number>;
    borderRadius?: Val<number>;
    borderPosition?: Val<number>;
    shadowColor?: Val<unknown>;
    shadowOffset?: Val<[number, number]>;
    shadowBlur?: Val<number>;
    alignment?: Val<number>;
    queryKey?: Val<string[]>;
    children?: EdgyElement[];
}

export function Container(props: ContainerProps): EdgyElement {
    return __tur.Container(__ctx, props);
}

export function SizedBox(props: {
    width?: Val<number>;
    height?: Val<number>;
    children?: EdgyElement[];
}): EdgyElement {
    return __tur.Container(__ctx, props);
}

export interface FlexProps {
    mainAlignment?: Val<number>;
    crossAlignment?: Val<number>;
    mainAxisSize?: Val<number>;
    children: EdgyElement[];
}

export function Column(props: FlexProps): EdgyElement {
    return __tur.Column(__ctx, props);
}

export function Row(props: FlexProps): EdgyElement {
    return __tur.Row(__ctx, props);
}

export function Expanded(props: {
    flex?: Val<number>;
    child: EdgyElement;
}): EdgyElement {
    return __tur.Expanded(__ctx, props);
}

export function Stack(props: { children: EdgyElement[] }): EdgyElement {
    return __tur.Stack(__ctx, props);
}

export interface PositionedProps {
    left?: Val<number>;
    top?: Val<number>;
    right?: Val<number>;
    bottom?: Val<number>;
    width?: Val<number>;
    height?: Val<number>;
    child: EdgyElement;
}

export function Positioned(props: PositionedProps): EdgyElement {
    return __tur.Positioned(__ctx, props);
}

export interface TextProps {
    text: Val<string>;
    fontSize?: Val<number>;
    color?: Val<unknown>;
    spans?: Val<unknown>;
    /**
     * When `true`, the text can be drag-selected with the pointer. Defaults
     * to `false` (matches the browser convention for `<span>`/`<div>` text).
     * Non-reactive — toggle by rebuilding the element.
     */
    selectable?: boolean;
    queryKey?: Val<string[]>;
}

export function Text(props: TextProps): EdgyElement {
    return __tur.Text(__ctx, props);
}

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

export interface PointerInteractProps {
    /** Click handler. Always receives a `PointerInteractEvent` with local
     *  and global positions. Callers that don't need the event can ignore it
     *  (`mutate((_ctx, _ev) => ...)`) — TS backward-infers `_ev`'s type from
     *  this prop. */
    onClick?: Mutation<[PointerInteractEvent]>;
    onPointerDown?: Mutation<[PointerInteractEvent]>;
    /** Fires on every pointer move while a drag is in progress (pointer is
     *  down). Hover moves (no button held) do NOT fire this — use
     *  `MouseRegion` for hover. */
    onPointerMove?: Mutation<[PointerInteractEvent]>;
    onPointerUp?: Mutation<[PointerInteractEvent]>;
    /** Right-click / context-menu. Fires with the local + global position
     *  of the click. Use this to show a context menu. */
    onContextMenu?: Mutation<[PointerInteractEvent]>;
    behavior?: Val<number>;
    queryKey?: Val<string[]>;
    child?: EdgyElement;
}

export function PointerInteract(props: PointerInteractProps): EdgyElement {
    return __tur.PointerInteract(__ctx, props);
}

// ---------------------------------------------------------------------------
// Cursor — the set of OS cursor styles (standard CSS cursor keywords). Mirrors
// the `tur_shared::Cursor` enum; the bridge decodes the keyword string.
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
// MouseRegion — declarative cursor + hover (enter/exit) callbacks. Mirrors
// Flutter's `MouseRegion`. Use this (NOT PointerInteract) when you need to
// change the OS cursor or detect pointer enter/exit. Use PointerInteract for
// click + drag.
// ---------------------------------------------------------------------------

export interface PointerRegionEvent {
    local: Point;
    global: Point;
}

export interface MouseRegionProps {
    /** OS cursor style. See {@link Cursor}. */
    cursor?: Val<Cursor>;
    onEnter?: Mutation<[PointerRegionEvent]>;
    onExit?: Mutation<[PointerRegionEvent]>;
    behavior?: Val<number>;
    child?: EdgyElement;
}

export function MouseRegion(props: MouseRegionProps): EdgyElement {
    return __tur.MouseRegion(__ctx, props);
}

export interface ConditionProps {
    condition: Val<boolean>;
    /** Then-branch thunk — built only when `condition` is truthy. */
    child?: () => EdgyElement;
    /** Else-branch thunk — built only when `condition` is falsy. */
    elseChild?: () => EdgyElement;
    queryKey?: Val<string[]>;
}

export function Condition(props: ConditionProps): EdgyElement {
    return __tur.Condition(__ctx, props);
}

export interface SwitchProps {
    /** Reactive key to match on (any value), or a static value. */
    value: Val<string | number | boolean | null | undefined>;
    /** Ordered list of `{ key, child }` entries. First match wins. Each `child`
     *  is a thunk built only when its case is selected. */
    cases: Array<{
        key: string | number | boolean | null | undefined;
        child: () => EdgyElement;
    }>;
    /** Mounted (built) when no case matches. */
    fallback?: () => EdgyElement;
    queryKey?: Val<string[]>;
}

export function Switch(props: SwitchProps): EdgyElement {
    return __tur.Switch(__ctx, props);
}

export interface ScrollViewProps {
    axis?: Val<number>;
    padding?: Val<number>;
    color?: Val<unknown>;
    controller?: unknown;
    child: EdgyElement;
    queryKey?: Val<string[]>;
}

export function ScrollView(props: ScrollViewProps): EdgyElement {
    return __tur.ScrollView(__ctx, props);
}

export interface ScrollbarProps {
    /** A `ScrollController` shared with a `ScrollView`. */
    controller?: unknown;
    /** Thumb brush (color). Defaults to a semi-transparent gray. */
    color?: Val<unknown>;
    /** Track background brush (painted behind the thumb). Omit for no track. */
    trackColor?: Val<unknown>;
    /** Track thickness (width for a vertical scrollbar). Defaults to 10. */
    thickness?: Val<number>;
    /** Thumb corner radius. Defaults to half the track width. */
    thumbRadius?: Val<number>;
    queryKey?: Val<string[]>;
}

export function Scrollbar(props: ScrollbarProps): EdgyElement {
    return __tur.Scrollbar(__ctx, props);
}

export interface LazyListProps {
    axis?: Val<number>;
    itemCount: Val<number>;
    overscan?: Val<number>;
    /** Fixed size (along the main axis) for every item. When provided, the
     *  visible-range math is exact and the list can virtualize at very large
     *  `itemCount`s (e.g. 10,000+). Omit for variable-height items — the
     *  average of measured children is used as a fallback. */
    itemExtent?: Val<number>;
    builder: (index: number) => EdgyElement;
    queryKey?: Val<string[]>;
}

export function LazyList(props: LazyListProps): EdgyElement {
    return __tur.LazyList(__ctx, props);
}

export interface EachProps<T> {
    /** Reactive array (atom) of items. The list is rebuilt when this changes. */
    items: Readable<T[]>;
    /** Build one element per item. */
    build: (item: T, index: number) => EdgyElement;
    mainAlignment?: Val<MainAxisAlignment>;
    crossAlignment?: Val<CrossAxisAlignment>;
    mainAxisSize?: Val<MainAxisSize>;
    queryKey?: Val<string[]>;
}

export function Each<T>(props: EachProps<T>): EdgyElement {
    return __tur.Each(__ctx, props);
}

export function ImageEdgy(props: {
    resourceId: Val<number>;
    width?: Val<number>;
    height?: Val<number>;
    fit?: Val<number>;
    queryKey?: Val<string[]>;
    child?: EdgyElement;
}): EdgyElement {
    return __tur.ImageEdgy(__ctx, props);
}

export function InputEdgy(props: {
    controller?: unknown;
    /** Optional `UndoController` (created via `createUndoController()`) that
     *  enables Cmd/Ctrl+Z + Cmd/Ctrl+Shift+Z (and Ctrl+Y) keyboard shortcuts.
     *  The controller is shared across rebuilds — pass the same instance. */
    undoController?: unknown;
    placeholder?: Val<string>;
    color?: Val<unknown>;
    placeholderColor?: Val<unknown>;
    cursorColor?: Val<unknown>;
    fontSize?: Val<number>;
    fontFamily?: Val<string>;
    width?: Val<number>;
    height?: Val<number>;
    multiline?: Val<boolean>;
    onContextMenu?: Mutation<[PointerInteractEvent]>;
    queryKey?: Val<string[]>;
}): EdgyElement {
    return __tur.InputEdgy(__ctx, props);
}

export function Fragment(props: { children: EdgyElement[] }): EdgyElement {
    return __tur.Fragment(__ctx, props);
}

// ---------------------------------------------------------------------------
// Opacity / Transform — apply visual effects to a subtree.
//
// `Opacity({ value, child })` multiplies the child's alpha by `value` (0..1).
// `Transform({ scale, rotate, translateX, translateY, child })` applies a 2D
// affine transform. Both are reactive and integrate with `createAnimationController`
// — animate the `value` / `scale` / etc. via `onTick` for fade/scale/rotate
// transitions.
// ---------------------------------------------------------------------------

export function Opacity(props: {
    value: Val<number>;
    child?: EdgyElement;
    queryKey?: Val<string[]>;
}): EdgyElement {
    return __tur.Opacity(__ctx, props);
}

export interface TransformProps {
    /** Uniform scale (multiplies both X and Y). */
    scale?: Val<number>;
    /** Per-axis scale. Overrides `scale` for that axis when present. */
    scaleX?: Val<number>;
    scaleY?: Val<number>;
    /** Rotation in radians (clockwise). */
    rotate?: Val<number>;
    translateX?: Val<number>;
    translateY?: Val<number>;
    child?: EdgyElement;
    queryKey?: Val<string[]>;
}

export function Transform(props: TransformProps): EdgyElement {
    return __tur.Transform(__ctx, props);
}

// ---------------------------------------------------------------------------
// Controllers (reuse existing bridge factories).
//
// Every callback is a `Mutation` atom (created via `mutate(fn)`). At invoke
// time the engine prepends the `{ get, set }` store context as the first
// argument, so each callback's `Args` describes only the event payload.
// ---------------------------------------------------------------------------

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

export interface TextEditingControllerOpts {
    /** Initial text shown when an input first mounts this controller. The
     *  value is set at construction time and replaces the default empty
     *  buffer — useful for modals that pre-fill an editable field with
     *  the current value of whatever is being edited. */
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

export interface ScrollControllerOpts {
    onScroll?: Mutation<[ScrollEvent], void>;
    initialOffset?: number;
}

export interface LazyListControllerOpts {
    onScroll?: Mutation<[ScrollEvent], void>;
    onVisibleRangeChange?: Mutation<[number, number], void>;
}

export function createTextEditingController(
    opts: TextEditingControllerOpts = {},
): unknown {
    return __tur.createTextEditingController(__ctx, opts);
}

/**
 * `UndoController` — Flutter-style undo/redo history stack. Pair with a
 * `TextEditingController` via the `InputEdgy({ undoController })` prop to
 * enable Cmd/Ctrl+Z and Cmd/Ctrl+Shift+Z (plus Ctrl+Y on Windows). The
 * controller's `canUndo` / `canRedo` getters are reactive-friendly — read
 * them whenever a menu needs to enable/disable its Undo/Redo items.
 */
export interface UndoController {
    /** True when at least one earlier state can be restored. */
    readonly canUndo: boolean;
    /** True when at least one later state can be re-applied. */
    readonly canRedo: boolean;
    /** Reset both stacks. */
    clear(): void;
}

export function createUndoController(): UndoController {
    return __tur.createUndoController(__ctx) as UndoController;
}

export function createScrollController(
    opts: ScrollControllerOpts = {},
): unknown {
    return __tur.createScrollController(__ctx, opts);
}

export function createLazyListController(
    opts: LazyListControllerOpts = {},
): unknown {
    return __tur.createLazyListController(__ctx, opts);
}

export type AnimationStatus =
    | "stopped"
    | "forward"
    | "reverse"
    | "completed"
    | "paused";

export interface AnimationController {
    /** Current raw (un-eased) progress 0..1. */
    readonly value: number;
    /** Current status. */
    readonly status: AnimationStatus;
    /** Duration in milliseconds. */
    readonly duration: number;
    /** Current speed multiplier (default 1.0). */
    readonly speed: number;
    /** Play forward from 0 to 1. Resets value to 0. */
    forward(): void;
    /** Play reverse from 1 to 0. Resets value to 1. */
    reverse(): void;
    /** Stop and freeze the current value. Status becomes "stopped". */
    stop(): void;
    /** Pause if currently playing. Status becomes "paused". `resume()`
     *  continues from the frozen value. */
    pause(): void;
    /** Resume from a paused state, continuing in the original direction. */
    resume(): void;
    /** Jump to a specific progress value (0..1). If playing, continues from
     *  there; otherwise freezes at the new value. Fires `onTick`. */
    seek(t: number): void;
    /** Set the speed multiplier (e.g. 0.5 = half speed, 2.0 = double).
     *  Must be positive. If currently playing, the value is aligned before
     *  the new speed takes effect. */
    setSpeed(factor: number): void;
    /** Set the repeat count. Pass a positive integer for finite iterations
     *  (the animation transitions to status "completed" after the count),
     *  or the string `"infinite"` to loop forever. Default is 1. */
    repeat(count: number | "infinite"): void;
}

export interface AnimationControllerOpts {
    duration?: number;
    curve?: "linear" | "easeIn" | "easeOut" | "easeInOut";
    /** Repeat policy. A positive integer plays exactly that many iterations
     *  then completes; the string `"infinite"` loops forever (status stays
     *  "forward"/"reverse", `onEnd` never fires). Default is 1. */
    repeat?: number | "infinite";
    /** Fired each frame with the eased progress (0..1). The callback is a
     *  `Mutation<[number], void>` — dispatched via the engine's mutation
     *  queue, so it fires after any active `RefMut` borrow on the
     *  controller is released. This lets the callback safely read
     *  `ctrl.status` / `ctrl.value` without a boa `BorrowError`. */
    onTick?: Mutation<[number], void>;
    /** Fired once when the animation completes. Same dispatch contract as
     *  `onTick`. Never fires when `repeat: "infinite"`. */
    onEnd?: Mutation<[], void>;
}

export function createAnimationController(
    opts: AnimationControllerOpts = {},
): AnimationController {
    return __tur.createAnimationController(__ctx, opts) as AnimationController;
}

export function createImageResource(bytes: Uint8Array | ArrayBuffer): number {
    return __tur.createImageResource(__ctx, bytes);
}

/** Parse and rasterise an inline SVG string into an image resource. Returns
 *  a resource id compatible with `ImageEdgy` — SVGs are just another kind of
 *  image at runtime, rendered to pixels up front at the document's declared
 *  size. */
export function createSvgResource(svg: string): number {
    return __tur.createSvgResource(__ctx, svg);
}

// ---------------------------------------------------------------------------
// Render — mount a component tree.
// ---------------------------------------------------------------------------

export function render(comp: EdgyComponent): void {
    __tur.render(__ctx, comp);
}

export * from "./color";

// ---------------------------------------------------------------------------
// Enums (mirror of `tur-shared`).
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

// ---------------------------------------------------------------------------
// Global type declaration.
// ---------------------------------------------------------------------------

interface TurGlobal {
    source(ctx: unknown, value: unknown): unknown;
    derive(ctx: unknown, fn: (ctx: StoreCtx) => unknown): unknown;
    mutate(
        ctx: unknown,
        fn: (ctx: StoreCtx, ...args: unknown[]) => unknown,
    ): unknown;
    get(ctx: unknown, a: unknown): unknown;
    set(ctx: unknown, target: unknown, ...rest: unknown[]): unknown;
    component(ctx: unknown, f: () => EdgyElement): EdgyElement;

    Container(ctx: unknown, props: unknown): EdgyElement;
    Column(ctx: unknown, props: unknown): EdgyElement;
    Row(ctx: unknown, props: unknown): EdgyElement;
    Expanded(ctx: unknown, props: unknown): EdgyElement;
    Stack(ctx: unknown, props: unknown): EdgyElement;
    Positioned(ctx: unknown, props: unknown): EdgyElement;
    Text(ctx: unknown, props: unknown): EdgyElement;
    PointerInteract(ctx: unknown, props: unknown): EdgyElement;
    MouseRegion(ctx: unknown, props: unknown): EdgyElement;
    Condition(ctx: unknown, props: unknown): EdgyElement;
    Switch(ctx: unknown, props: unknown): EdgyElement;
    ScrollView(ctx: unknown, props: unknown): EdgyElement;
    Scrollbar(ctx: unknown, props: unknown): EdgyElement;
    LazyList(ctx: unknown, props: unknown): EdgyElement;
    Each(ctx: unknown, props: unknown): EdgyElement;
    ImageEdgy(ctx: unknown, props: unknown): EdgyElement;
    InputEdgy(ctx: unknown, props: unknown): EdgyElement;
    Fragment(ctx: unknown, props: unknown): EdgyElement;
    Opacity(ctx: unknown, props: unknown): EdgyElement;
    Transform(ctx: unknown, props: unknown): EdgyElement;

    render(ctx: unknown, root: EdgyElement): void;

    createTextEditingController(ctx: unknown, opts: unknown): unknown;
    createUndoController(ctx: unknown): unknown;
    createScrollController(ctx: unknown, opts: unknown): unknown;
    createLazyListController(ctx: unknown, opts: unknown): unknown;
    createAnimationController(ctx: unknown, opts: unknown): unknown;
    createImageResource(ctx: unknown, bytes: Uint8Array | ArrayBuffer): number;
    createSvgResource(ctx: unknown, svg: string): number;
}
