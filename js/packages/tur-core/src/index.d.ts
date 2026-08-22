/**
 * @tur-ng/core — ambient type declarations for the native tur reactive core.
 *
 * Runtime is a synthetic boa module registered by tur-engine
 * (`core::js_runtime::module_loader`, wired in `core::runtime`) under the
 * specifier `"tur:core"`. It exports the reactive substrate + view entry:
 * atom declarations (`source`/`derive`/`mutate` — pure handles carrying no
 * state), `view`, and the `mount(view)` view-tree entry point, plus the
 * opaque meta-types (`Element`/`Source`/`Derived`/`Mutation`/`Readable`/
 * `Val`/`Store`/`ReadonlyStoreCtx`/`StoreCtx`).
 *
 * This is the authoritative contract for the engine's reactive layer. The
 * widget library (`tur:std`, declared in `@tur-ng/std`) re-exports
 * everything here and adds views, value types, enums, and event details.
 * Consumers normally import from `tur:std`; `@tur-ng/animation` and
 * other low-level libraries may import directly from `tur:core`.
 *
 * Handles (`Element`, `Source`, `Derived`, `Mutation`, `Store`) are opaque —
 * the engine hands out Rust-owned `JsObject` opaques; callers must treat
 * them as opaque.
 *
 * THE STORE IS THE KV: `source(v)` / `derive(fn)` / `mutate(fn)` return pure
 * declarations — no state is stored. The instance store holds the values;
 * reading/writing a declaration through it materializes the value there.
 * Each instance has exactly ONE store — created by the engine and handed to
 * the module's `start({ store })`; there is no `createStore`. The module
 * mounts its tree with `mount(view)` against that store. Engine-minted
 * atoms (e.g. `viewportSize$`) live in the same store and resolve from every
 * read path.
 *
 * THERE IS NO "CURRENT STORE" GETTER: reactive access happens through the
 * ctx handed to `derive`/`mutate` closures (and callback props like
 * `onTick`/`onClick`, which are mutations). The ctx is a stable store-bound
 * reader/writer, so code that needs reactive access outside a closure —
 * helper functions, `launch` generator bodies — threads/captures the ctx
 * from the enclosing mutation instead:
 *
 *     const startLoop = mutate((ctx) => {
 *         launch(function* () {
 *             while (ctx.get(running$)) { yield sleep(1000); ctx.set(n$, ...); }
 *         });
 *     });
 *
 * Side-effecting helpers are declared as mutations themselves and composed
 * by dispatch: `ctx.set(action, ...args)` (or `store.set(action, ...args)`
 * from outside). The engine's flush is a fixed-point loop, so a mutation
 * dispatched from inside another mutation runs within the same frame:
 *
 *     const save = mutate((ctx, name) => { ... });
 *     const commit = mutate((ctx) => { ...; ctx.set(save, name); });
 *
 * The event framework is two functions: `mutate` (declare a handler as a
 * deferred `Mutation` atom) and `store.set` (dispatch it). The concrete
 * event payload shapes (`PointerInteractEvent`, `KeyEvent`, …) live in
 * `tur:std` — core is event-type-agnostic.
 *
 * `watch(atom, cb)` subscribes a mutation to an atom OUTSIDE the element
 * tree — the non-element counterpart of `ReadableSubscribe`. `cb` is a
 * `mutate((ctx) => …)` handle (the same convention as `onTick`). It
 * returns `{ start$, stop$ }` mutations: dispatch `start$` to begin
 * delivery (`store.set(handle.start$)`, or hand it to `lifecycleView` as
 * `onMounted$`) and `stop$` to end it (`beforeDestroy$`). Change-only:
 * starting does NOT fire the callback; it fires when the watched atom is
 * dirtied, at most once per frame. Watchers must not write the atom they
 * watch (directly or through a derived's dep) — the engine throws a
 * "watch loop detected" error at the offending `set` call site.
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

    /** A reactive atom declaration holding a value of type `T` — created via
     *  `source()`. Carries no state: the initial value seeds whichever store
     *  first materializes the declaration. `T` is recovered at the call site
     *  by the generic primitives (`store.get`, `store.set`) — no runtime
     *  field. */
    export interface Source<T> {}

    /** A computed atom declaration holding a value of type `T` — created via
     *  `derive()`. Its value is recomputed by the engine from its declared
     *  dependencies (tracked automatically from closure reads);
     *  `store.set` rejects derived atoms at runtime. */
    export interface Derived<T> {}

    /** A mutation atom: a deferred callback `(ctx, ...Args) => R`. This is the
     *  event-handler type — `mutate` creates one, `store.set` invokes it. */
    export interface Mutation<Args extends unknown[] = [], R = void> {}

    /** Anything you can read a current value from (a source or derived atom). */
    export type Readable<T> = Source<T> | Derived<T>;

    /** A value-or-reactive: either a plain `T` or a `Readable<T>`. The engine
     *  re-reads reactives each layout pass; plain values are fixed at build time. */
    export type Val<T> = T | Readable<T>;

    // ---------------------------------------------------------------------------
    // Store — the KV container for atom state. One per instance: created by
    // the engine and handed to the module's `start({ store })`. Opaque
    // handle; reads/writes accept declarations (materialized into THIS
    // store) and engine-owned atoms.
    // ---------------------------------------------------------------------------

    export interface Store {
        get<T>(a: Readable<T>): T;
        set<T>(s: Source<T>, value: T): void;
        set<Args extends unknown[], R>(m: Mutation<Args, R>, ...args: Args): R;
    }

    // ---------------------------------------------------------------------------
    // Store context — handed to `derive` / `mutate` closures as their first
    // arg. `derive` is pure (read-only); `mutate` (and other side-effecting
    // callbacks) may also write. The split is type-level only — the runtime
    // ctx object is the same `{ get, set }`; this just guides callers away
    // from calling `set` inside a `derive` (which could trigger a recompute
    // loop).
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
    // Reactive primitives — declarations only; no state is stored until a
    // store materializes them.
    // ---------------------------------------------------------------------------

    export function source<T>(value: T): Source<T>;
    export function derive<T>(fn: (ctx: ReadonlyStoreCtx) => T): Derived<T>;
    export function mutate<Args extends unknown[], R>(
        fn: (ctx: StoreCtx, ...args: Args) => R,
    ): Mutation<Args, R>;

    // ---------------------------------------------------------------------------
    // watch — non-element subscription over an atom. The counterpart of
    // `ReadableSubscribe` for state flows that live outside the view tree:
    // fetch-on-change, persistence, loggers, derived side effects.
    // ---------------------------------------------------------------------------

    /** Control handle returned by `watch` — both fields are mutations.
     *  `start$` begins delivery (idempotent; does NOT fire the callback),
     *  `stop$` ends it (idempotent). Dispatch them like any mutation:
     *  `store.set(handle.start$)` / `ctx.set(handle.start$)`, or wire them
     *  straight into `lifecycleView` (`onMounted$: start$`,
     *  `beforeDestroy$: stop$`) so the watcher lives exactly as long as the
     *  tree that owns it. */
    export interface WatchHandle {
        start$: Mutation<[], void>;
        stop$: Mutation<[], void>;
    }

    /** Subscribe `cb` — a mutation handle (`mutate((ctx) => …)`, the same
     *  convention as `onTick` / `onUpdate$`) — to a source or derived atom.
     *  While started, the engine invokes it with the store ctx whenever the
     *  watched atom is dirtied — change-only (starting never fires it), at
     *  most once per frame (same-frame coalescing), same rail as every other
     *  mutation.
     *
     *  Writes are equality-gated per atom value; note a derived whose dep
     *  changes but which recomputes to the same value still counts as
     *  dirtied (delivery is invalidation-based, not value-based).
     *
     *  LOOP RULE: the callback must not write the watched atom — directly,
     *  or by writing a dep of a watched derived. The engine throws a
     *  "watch loop detected" error at the offending `set` call site and
     *  rejects the write. For a fetch-style flow, write results to a
     *  separate result atom; to force a refetch, bump a nonce inside the
     *  watched trigger object (writes compare object values by reference,
     *  so a fresh `{ query, nonce }` object always triggers).
     *
     *  `watch` is a pure declaration (like `source`/`mutate`): registration
     *  happens at call time, but nothing is delivered until `start$`. */
    export function watch<T>(
        atom: Readable<T>,
        cb: Mutation<[], void>,
    ): WatchHandle;

    export function view(f: () => Element): Element;

    // ---------------------------------------------------------------------------
    // Mounting
    // ---------------------------------------------------------------------------

    /** Mount the view tree's root into the instance-owned tree, against the
     *  instance store (the one `start({ store })` received — declarations in
     *  the tree materialize into it). Replaces any previously mounted
     *  root. */
    export function mount(root: Element): void;
}

// Explicit re-exports for consumers importing the package directly
// (not via the ambient "tur:core" module specifier).
export type {
    Derived,
    Element,
    Mutation,
    Readable,
    ReadonlyStoreCtx,
    Source,
    Store,
    StoreCtx,
    Val,
    WatchHandle,
} from "tur:core";
