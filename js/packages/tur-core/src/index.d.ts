/**
 * @tur/core — ambient type declarations for the native tur reactive core.
 *
 * Runtime is a synthetic boa module registered by tur-engine
 * (`core::bridge::module_loader`) under the specifier `"builtin:tur/core"`.
 * It exports only the reactive substrate + event framework: atom primitives
 * (`source`/`derive`/`mutate`/`get`/`set`/`view`), the `render` mount entry
 * point, and the opaque meta-types (`Element`/`Atom`/`Mutation`/`Readable`/
 * `Val`/`ReadonlyStoreCtx`/`StoreCtx`).
 *
 * This is the authoritative contract for the engine's reactive layer. The
 * widget library (`builtin:tur/std`, declared in `@tur/std`) re-exports
 * everything here and adds views, value types, enums, and event details.
 * Consumers normally import from `builtin:tur/std`; `@tur/animation` and
 * other low-level libraries may import directly from `builtin:tur/core`.
 *
 * Handles (`Element`, `Atom`, `Mutation`) are opaque — the engine hands out
 * Rust-owned `JsObject` opaques; callers must treat them as opaque.
 *
 * The event framework is two functions: `mutate` (declare a handler as a
 * deferred `Mutation` atom) and `set` (dispatch it). The concrete event
 * payload shapes (`PointerInteractEvent`, `KeyEvent`, …) live in
 * `builtin:tur/std` — core is event-type-agnostic.
 *
 * `derive` callbacks receive a `ReadonlyStoreCtx` (get-only); `mutate` and
 * other side-effecting callbacks receive the full `StoreCtx` (get + set).
 */

declare module "builtin:tur/core" {
    // ---------------------------------------------------------------------------
    // Opaque handles
    // ---------------------------------------------------------------------------

    /** An element handle returned by a view factory (`Container`, `Column`, …).
     *  Opaque — the engine owns the underlying `ElementTree` node. */
    export interface Element {}

    /** A writable reactive atom holding a value of type `T`. `T` is recovered at
     *  the call site by the generic primitives (`get`, `set`) — no runtime field. */
    export interface Atom<T> {}

    /** A mutation atom: a deferred callback `(ctx, ...Args) => R`. This is the
     *  event-handler type — `mutate` creates one, `set` invokes it. */
    export interface Mutation<Args extends unknown[] = [], R = void> {}

    /** Anything you can read a current value from (an `Atom` or derived atom). */
    export type Readable<T> = Atom<T>;

    /** A value-or-reactive: either a plain `T` or a `Readable<T>`. The engine
     *  re-reads reactives each layout pass; plain values are fixed at build time. */
    export type Val<T> = T | Readable<T>;

    // ---------------------------------------------------------------------------
    // Store context — handed to `derive` / `mutate` closures as their first arg.
    // `derive` is pure (read-only); `mutate` (and other side-effecting callbacks)
    // may also write. The split is type-level only — the runtime ctx object is the
    // same `{ get, set }`; this just guides callers away from calling `set` inside
    // a `derive` (which could trigger a recompute loop).
    // ---------------------------------------------------------------------------

    /** Read-only view of the store context. Handed to `derive` closures. */
    export interface ReadonlyStoreCtx {
        get<T>(a: Readable<T>): T;
    }

    /** Read/write store context. Handed to `mutate` and other side-effecting
     *  closures (`onTick`, event handlers, …). Extends `ReadonlyStoreCtx`. */
    export interface StoreCtx extends ReadonlyStoreCtx {
        set<T>(s: Atom<T>, value: T): void;
        set<Args extends unknown[], R>(m: Mutation<Args, R>, ...args: Args): R;
    }

    // ---------------------------------------------------------------------------
    // Reactive primitives
    // ---------------------------------------------------------------------------

    export function source<T>(value: T): Atom<T>;
    export function derive<T>(fn: (ctx: ReadonlyStoreCtx) => T): Readable<T>;
    export function mutate<Args extends unknown[], R>(
        fn: (ctx: StoreCtx, ...args: Args) => R,
    ): Mutation<Args, R>;
    export function get<T>(a: Readable<T>): T;
    export function set<T>(s: Atom<T>, value: T): void;
    export function set<Args extends unknown[], R>(
        m: Mutation<Args, R>,
        ...args: Args
    ): R;
    export function view(f: () => Element): Element;

    // ---------------------------------------------------------------------------
    // Mounting
    // ---------------------------------------------------------------------------

    export function render(root: Element): void;
}
