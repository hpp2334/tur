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
    queryKey?: Val<string[]>;
}

export function Text(props: TextProps): EdgyElement {
    return __tur.Text(__ctx, props);
}

export interface PointerInteractProps {
    onClick?: Mutation;
    onPointerEnter?: Mutation;
    onPointerExit?: Mutation;
    behavior?: Val<number>;
    child?: EdgyElement;
}

export function PointerInteract(props: PointerInteractProps): EdgyElement {
    return __tur.PointerInteract(__ctx, props);
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

export interface LazyListProps {
    axis?: Val<number>;
    itemCount: Val<number>;
    overscan?: Val<number>;
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
    placeholder?: Val<string>;
    color?: Val<unknown>;
    placeholderColor?: Val<unknown>;
    cursorColor?: Val<unknown>;
    fontSize?: Val<number>;
    fontFamily?: Val<string>;
    width?: Val<number>;
    height?: Val<number>;
    multiline?: Val<boolean>;
    queryKey?: Val<string[]>;
}): EdgyElement {
    return __tur.InputEdgy(__ctx, props);
}

export function Fragment(props: { children: EdgyElement[] }): EdgyElement {
    return __tur.Fragment(__ctx, props);
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

export function createAnimationController(
    opts: Record<string, unknown> = {},
): unknown {
    return __tur.createAnimationController(__ctx, opts);
}

export function createImageResource(bytes: Uint8Array | ArrayBuffer): number {
    return __tur.createImageResource(__ctx, bytes);
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
    Condition(ctx: unknown, props: unknown): EdgyElement;
    Switch(ctx: unknown, props: unknown): EdgyElement;
    ScrollView(ctx: unknown, props: unknown): EdgyElement;
    LazyList(ctx: unknown, props: unknown): EdgyElement;
    Each(ctx: unknown, props: unknown): EdgyElement;
    ImageEdgy(ctx: unknown, props: unknown): EdgyElement;
    InputEdgy(ctx: unknown, props: unknown): EdgyElement;
    Fragment(ctx: unknown, props: unknown): EdgyElement;

    render(ctx: unknown, root: EdgyElement): void;

    createTextEditingController(ctx: unknown, opts: unknown): unknown;
    createScrollController(ctx: unknown, opts: unknown): unknown;
    createLazyListController(ctx: unknown, opts: unknown): unknown;
    createAnimationController(ctx: unknown, opts: unknown): unknown;
    createImageResource(ctx: unknown, bytes: Uint8Array | ArrayBuffer): number;
}
