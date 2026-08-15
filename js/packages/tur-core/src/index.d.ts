/**
 * @tur-ng/core — ambient type declarations for the native tur reactive core.
 *
 * Runtime is a synthetic boa module registered by tur-engine
 * (`core::bridge::module_loader`) under the specifier `"tur:core"`.
 * It exports only the reactive substrate + event framework: atom primitives
 * (`source`/`derive`/`mutate`/`get`/`set`/`view`), the view-root mount API
 * (`viewRoot`/`viewRoots`/`setViewRoot`/`resetViewRoot`), and the opaque
 * meta-types (`Element`/`Source`/`Derived`/`Mutation`/`Readable`/`Val`/
 * `ReadonlyStoreCtx`/`StoreCx`/`ViewRoot`).
 *
 * This is the authoritative contract for the engine's reactive layer. The
 * widget library (`tur:std`, declared in `@tur-ng/std`) re-exports
 * everything here and adds views, value types, enums, and event details.
 * Consumers normally import from `tur:std`; `@tur-ng/animation` and
 * other low-level libraries may import directly from `tur:core`.
 *
 * Handles (`Element`, `Source`, `Derived`, `Mutation`) are opaque — the engine
 * hands out Rust-owned `JsObject` opaques; callers must treat them as opaque.
 *
 * The event framework is two functions: `mutate` (declare a handler as a
 * deferred `Mutation` atom) and `set` (dispatch it). The concrete event
 * payload shapes (`PointerInteractEvent`, `KeyEvent`, …) live in
 * `tur:std` — core is event-type-agnostic.
 *
 * `derive` callbacks receive a `ReadonlyStoreCtx` (get-only); `mutate` and
 * other side-effecting callbacks receive the full `StoreCtx` (get + set).
 */

declare module "tur:core" {
    // ---------------------------------------------------------------------------
    // Opaque handles
    // ---------------------------------------------------------------------------

    /** An element handle returned by a view factory (`Container`, `Column`, …).
     *  Opaque — the engine owns the underlying `ElementTree` node. */
    export interface Element {}

    /** A writable reactive atom holding a value of type `T` — created via
     *  `source()`. `T` is recovered at the call site by the generic primitives
     *  (`get`, `set`) — no runtime field. */
    export interface Source<T> {}

    /** A read-only computed atom holding a value of type `T` — created via
     *  `derive()`. Its value is recomputed by the engine from its declared
     *  dependencies; `set` rejects derived atoms at runtime. */
    export interface Derived<T> {}

    /** A mutation atom: a deferred callback `(ctx, ...Args) => R`. This is the
     *  event-handler type — `mutate` creates one, `set` invokes it. */
    export interface Mutation<Args extends unknown[] = [], R = void> {}

    /** Anything you can read a current value from (a source or derived atom). */
    export type Readable<T> = Source<T> | Derived<T>;

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
        set<T>(s: Source<T>, value: T): void;
        set<Args extends unknown[], R>(m: Mutation<Args, R>, ...args: Args): R;
    }

    // ---------------------------------------------------------------------------
    // Reactive primitives
    // ---------------------------------------------------------------------------

    export function source<T>(value: T): Source<T>;
    export function derive<T>(fn: (ctx: ReadonlyStoreCtx) => T): Derived<T>;
    export function mutate<Args extends unknown[], R>(
        fn: (ctx: StoreCtx, ...args: Args) => R,
    ): Mutation<Args, R>;
    export function get<T>(a: Readable<T>): T;
    export function set<T>(s: Source<T>, value: T): void;
    export function set<Args extends unknown[], R>(
        m: Mutation<Args, R>,
        ...args: Args
    ): R;
    export function view(f: () => Element): Element;

    // ---------------------------------------------------------------------------
    // View roots — mounting
    //
    // One view root per host-registered surface (canvas/window). JS resolves
    // a root by name (`viewRoot("main")`), mounts (or replaces) its view via
    // `setViewRoot`, and reads the root's per-root size atom
    // (`root.viewportSize$`) + host-written lifecycle mirror (`root.active$`).
    // ---------------------------------------------------------------------------

    /** Opaque handle to one view root, obtained via `viewRoot(name)`. Exposes
     *  `name`, the per-root size atom `viewportSize$` (`{width, height}` in
     *  CSS pixels), and the host-written `active$` mirror (`true` while the
     *  root is set up; torn-down roots read `false`). */
    export interface ViewRoot {
        readonly name: string;
        readonly viewportSize$: Source<{ width: number; height: number }>;
        readonly active$: Source<boolean>;
    }

    /** Resolve a registered view root by name. Throws on an unknown name. */
    export function viewRoot(name: string): ViewRoot;

    /** All registered view-root names, in host declaration order. */
    export function viewRoots(): string[];

    /** Mount (or replace) a view root's view. Replacing destroys the previous
     *  subtree first (unmount hooks fire on the next flush). Mounting while
     *  the root is torn down records the intent only — the build is deferred
     *  until the host sets the root up again. */
    export function setViewRoot(root: ViewRoot, view: Element): void;

    /** Unmount the root's built tree AND clear the mount intent (a later
     *  host `setup_root` finds nothing to rebuild). */
    export function resetViewRoot(root: ViewRoot): void;
}
