# Using tur — the agent cookbook

Hands-on guide for **authoring and running tur apps** in this repo: the JS
module API, the reactive substrate, widgets, async, capabilities, and the
build/verify loop. Architecture deep-dives live in `AGENTS.md`; this
doc is the "how do I write one" companion.

Use this when: writing or editing a tur module/case, importing from
`tur:std` / `tur:core` / `tur:animation`, wiring `source`/`derive`/`mutate`,
the `start({ store })` / `mount` contract, `Task`/`sleep`/cancel idioms,
`tur:clipboard`/`tur:filepicker`, adding a test case, or running the
typecheck/test workflow.

## 1. What tur is

tur is a JavaScript rendering engine written in Rust (boa_engine JS +
vello-hybrid GPU rendering) with a Flutter-like widget/layout model. An
**app is a plain ES module** that imports widgets from synthetic modules
(`tur:std`, `tur:animation`, `tur:net`, …) and exports a `start()` entry. The
embedder (website wasm host, Android app, integration-test harness) loads
the module, hands it the instance store, and drives layout + paint + input.
This repo contains the engine (`libs/tur-engine`), the animation crate, the
embedders, and ~60 example cases (`js/packages/tur-test-cases/cases`) — the
best documentation is reading those.

## 2. The module contract (must-follow)

A loaded module **MUST export `function start()`**:

```ts
export function start() {
    mount(App);          // builds the root tree against the instance store
}
```

- `start({ store })` receives the **instance store** (a live
  `{ get, set }`) when you need a writer at boot — see
  `demo/playground-view/src/index.ts` for the canonical entry.
- `start` may **return a cleanup function**. It runs before the next module
  load / at instance destroy. Dispose only your own non-tree resources
  (timers, subscriptions, controllers) — the tree teardown is engine-owned;
  there is no `unmount` export.
- `export default` is **rejected** with a "export `function start()`" error.
- A broken reload never kills the running app: the engine parses the new
  module first, then runs the old cleanup.

## 3. The reactive substrate

The **store is the KV**. `source(v)` / `derive(fn)` / `mutate(fn)` return
*pure declarations* — no state is stored at call time. Each instance has
exactly ONE store, created by the engine; atom values materialize into it on
first read/write. There is no `createStore` and no `getStore()`.

Reactive access happens **only through closure ctx** — `derive` closures get
a read-only ctx, `mutate` (and event-handler) closures get a read/write ctx:

```ts
const count$ = source(0);                                  // seed a value
const double$ = derive((ctx) => ctx.get(count$) * 2);      // computed (auto dep-tracked)
const inc$ = mutate((ctx) => ctx.set(count$, ctx.get(count$) + 1)); // action
```

- `store.set(derived$, …)` is rejected — you write sources only.
- Helpers / `async` bodies that need reactive access **thread the ctx** from
  the enclosing mutation; side-effecting helpers are declared as mutations
  and composed by dispatch: `ctx.set(action, …args)` — nested dispatches run
  within the same frame (the flush is a fixed-point loop).
- **`watch(atom, cb)`** subscribes a `mutate` handle to any source/derived
  outside the view tree; returns `{ start$, stop$ }` mutations (change-only,
  at most once per frame). Dispatch `start$` to begin, `stop$` to end — or
  wire them into `lifecycleView` as `onMounted$` / `beforeDestroy$`. A
  watcher must not write the atom it watches (engine throws "watch loop
  detected").
- Widget props accept **`Val<T>`** — a plain value (fixed at build) or any
  `Readable` (re-read each layout pass). Passing a derived/source makes the
  prop live.
- `viewportSize$` (from `tur:std`) is the engine-owned live canvas size:
  `get(viewportSize$).width`. Read-only.

### View functions run exactly once

`view(fn)` invokes the thunk **once**, when the element tree is built (the
root `App`: at `mount`). Prop updates never re-invoke your function — live
props are `Val<T>` atoms re-read on each layout pass and applied via
`set_prop`. There is no re-render, no diffing, no hooks; don't carry React's
render-model instincts over.

So most **local state can live inside the view function** — `source` atoms,
`mutate` actions, controllers, `let` task handles. Created once per mount,
stable for the tree's life:

```ts
const App = view(() => {
    const count$ = source(0);                 // local state — created once
    const inc$ = mutate((ctx) => ctx.set(count$, ctx.get(count$) + 1));
    return Column({ children: [/* uses count$, inc$ */] });
});
```

Helper factories that need the state take it as a **parameter** (an atom or a
small state object — see `countdown` / `jigsaw-puzzle` for the
`createXxxState()` bundle pattern); they stay plain functions returning
`Element`.

Keep state at **module level** only when it is shared across views/files
(`todolist`'s / `github-viewer`'s `state.ts`) or must **survive a subtree
rebuild**. Rebuild boundaries re-run their thunks, and a re-run mints fresh
atoms (seed values, new ids — old values don't carry over):

- `Condition` / `Switch` branch thunks — re-run when the branch swaps.
- `Each({ build })` — rebuilds **every** item subtree whenever the `items`
  reference changes.
- `LazyList` / `LazyGrid` `builder(i)` — runs per materialized item, re-runs
  on data change.
- `lifecycleView(() => …)` — once per mount of that node; re-runs if the
  node is re-created (e.g. inside a swapped branch).

Ephemeral per-item state is fine inside `build`; anything that must persist
goes above the rebuild boundary (the enclosing stable view or module level).

## 4. Quick starts

Each is a complete module — drop into
`js/packages/tur-test-cases/cases/<name>/index.ts` and it runs under the
test harness or any embedder.

### 4.1 Minimal app — counter (from `cases/counter`)

```ts
import {
    Alignment, Color, Column, Container, CrossAxisAlignment, derive,
    Expanded, MainAxisAlignment, mount, mutate, PointerInteract, Row,
    source, Text, view,
} from "tur:std";

function Button({ label, onClick }: { label: string; onClick: Parameters<typeof PointerInteract>[0]["onClick"] }) {
    return PointerInteract({
        onClick,
        child: Container({
            width: 100, height: 44, borderRadius: 8,
            color: Color.hex("#6366f1"), alignment: Alignment.Center,
            children: [Text({ text: label, fontSize: 18, color: Color.hex("#ffffff") })],
        }),
    });
}

const App = view(() => {
    // Local state — the view fn runs exactly once (at build), so this atom
    // is stable for the life of the tree.
    const count$ = source(0);

    return Expanded({
        child: Container({
            color: Color.hex("#f8fafc"),
            children: [
                Column({
                    mainAlignment: MainAxisAlignment.Center,
                    crossAlignment: CrossAxisAlignment.Center,
                    children: [
                        Text({
                            text: derive((ctx) => `Count: ${ctx.get(count$)}`),
                            queryKey: ["count"], fontSize: 36,
                            color: Color.hex("#1e293b"),
                        }),
                        Row({
                            mainAlignment: MainAxisAlignment.Center,
                            children: [
                                Button({
                                    label: "-1",
                                    onClick: mutate((ctx) => ctx.set(count$, ctx.get(count$) - 1)),
                                }),
                                Button({
                                    label: "+1",
                                    onClick: mutate((ctx) => ctx.set(count$, ctx.get(count$) + 1)),
                                }),
                            ],
                        }),
                    ],
                }),
            ],
        }),
    });
});

export function start() {
    mount(App);
}
```

### 4.2 List + `Each` + text input (from `cases/todolist`)

State shared across views/files lives in one file (`state.ts`), views in
another — the module-level home is for *shared* state (each instance
materializes its own values from the seeds):

```ts
// state.ts
import { createTextEditingController, mutate, source, type StoreCtx } from "tur:std";

export interface Task { title: string; completed: boolean; }
export const tasks$ = source<Task[]>([]);
export const draft$ = source("");

export const draftCtrl = createTextEditingController({
    onInput: mutate((ctx, text: string, _enter: boolean) => ctx.set(draft$, text)),
});

// Plain helper taking the ctx — callable from any mutation wrapper.
export function addTask(ctx: StoreCtx, title: string) {
    ctx.set(tasks$, [...ctx.get(tasks$), { title, completed: false }]); // fresh array!
}

export const submit$ = mutate((ctx) => {
    addTask(ctx, ctx.get(draft$));      // compose via plain fns + ctx
});
```

```ts
// views.ts — items MUST be a Readable<T[]>
Each({
    items: tasks$,
    build: (task, _index) => Row({ children: [
        Text({
            text: task.title,
            color: task.completed ? Color.hex("#94a3b8") : Color.hex("#0f172a"),
        }),
    ] }),
})
```

Pair `draftCtrl` with an `Input({ controller: draftCtrl, placeholder: "…" })`;
`onInput` receives `(text, enterPressed)`.

### 4.3 Cancellable async loop (from `cases/countdown`)

Async runs on the engine's executor; cancellation is per-`Task` (state is
view-local — `countdown` wraps this cluster in `createCountdownState()`
called inside its view fn):

```ts
import { isCancelError, mutate, sleep, source, type Task } from "tur:std";

// inside the view fn (runs once at build, so this is stable local state):
const remaining$ = source(60);
const running$ = source(false);
let tick: Task<void> | null = null;

const start$ = mutate((ctx) => {
    if (ctx.get(running$)) return;
    ctx.set(running$, true);
    (async () => {
        try {
            while (ctx.get(running$)) {
                tick = sleep(1000);
                await tick.promise;
                ctx.set(remaining$, ctx.get(remaining$) - 1);
            }
        } catch (e) {
            if (!isCancelError(e)) throw e;   // CancelError = clean stop
        }
    })();
});

const stop$ = mutate((ctx) => {
    tick?.cancel();                           // rejects the awaited sleep
    ctx.set(running$, false);
});
```

**Debounce idiom** — the no-op rejection handler IS the cancelled branch:

```ts
let t: Task<string> | undefined;
// on each keystroke:
t?.cancel();
t = clipboard.readText();
t.promise.then((s) => use(s), () => {});
```

### 4.4 HTTP fetch (from `demo/playground-view/cases/github-viewer`)

The response body is **always raw bytes** — decode with `decodeUtf8`. Fetch
from inside a mutation; the async body captures the ctx:

```ts
import { request } from "tur:net";
import { decodeUtf8, mutate, source } from "tur:std";

interface Repo { name: string; /* … */ }
const result$ = source<Repo[] | null>(null);
const error$ = source<string | null>(null);

const load$ = mutate((ctx) => {
    ctx.set(error$, null);
    (async () => {
        try {
            const r = await request({
                url: "https://api.github.com/orgs/hpp2334/repos",
            }).promise;
            if (r.status !== 200) {
                ctx.set(error$, `HTTP ${r.status} ${r.statusText}`);
                return;
            }
            ctx.set(result$, JSON.parse(decodeUtf8(r.body)));
        } catch (e) {
            ctx.set(error$, String(e));
        }
    })();
});
```

`tur:net` is registered only when the embedder provides an `Http` backend —
feature-detect with `typeof request === "function"` before importing it (see
`demo/playground-view/src/cases/optional-ns.*.ts`). For large downloads use
`requestStream({ url, backpressure: { value: 64, unit: "KB" } })` and
`for await (const chunk of resp.body)`; `task.cancel()` wire-aborts.

### 4.5 Image (from `cases/image-basic`)

Resources are created once and referenced by numeric id:

```ts
import { BoxFit, createImageResource, Image, mount, view } from "tur:std";

const pngBytes = new Uint8Array([/* …raw PNG bytes… */]);
const resource = createImageResource(pngBytes);       // or createSvgResource(svgString)

const App = view(() =>
    Image({ resourceId: resource, width: 200, height: 100, fit: BoxFit.Contain }),
);

export function start() {
    mount(App);
}
```

### 4.6 Implicit animation (from `cases/implicit-animations`)

Pass a target + `duration` — the element animates from its previous value
when the target changes. No controller, no lerp boilerplate:

```ts
import { AnimatedContainer } from "tur:animation";
import { Color, derive, mutate, source } from "tur:std";

// inside your view fn (runs once at build — stable local state):
const expanded$ = source(false);
const toggle$ = mutate((ctx) => ctx.set(expanded$, !ctx.get(expanded$)));

// inside your tree:
AnimatedContainer({
    width: 150,
    height: 160,
    borderRadius: derive((ctx) => (ctx.get(expanded$) ? 40 : 12)),
    color: derive((ctx) =>
        ctx.get(expanded$) ? Color.rgb(99, 102, 241) : Color.rgb(14, 165, 233)),
    duration: 600,
    curve: "easeInOut",
})
```

`AnimatedOpacity` / `AnimatedPositioned` follow the same shape. For
continuous loops / transport control use the explicit path:
`createAnimationController({ duration, curve, repeat, onTick, onEnd })`
driving a `source` via `onTick`, paired with `Tween({begin, end}).lerp(t)`
or `ColorTween` inside a `derive` (see `cases/complex-animation`).

### Idioms

- **`$` suffix** on atom handles (`count$`, `running$`).
- **Local state in the view fn** — view fns run once at build, so
  `source`/`mutate`/controllers/`let` handles inside are stable for the
  tree's life. Factories that need them take them as props (or a state
  object — `createXxxState()` in `countdown` / `complex-animation` /
  `jigsaw-puzzle`).
- **Module level only for shared state** — atoms used across views/files
  (`todolist`, `github-viewer`) or state that must survive subtree rebuilds
  (`Condition`/`Switch` swaps, `Each` rebuilds). Seeds are per-instance;
  the store materializes.
- **Controllers built once** — at module load or inside the view fn
  (`createTextEditingController`, `createScrollController`, …), passed via
  props.
- **`queryKey: ["name"]`** on interactive/text elements — stable ids for
  dev-tool targeting and integration tests.
- **`Condition` / `Switch`** for branching subtrees; **`Stack` + opaque
  `PointerInteract` backdrop** for modals (see `cases/countdown`).
- **Helper factories are plain functions** returning `Element` (like
  `Button` above / `PrimaryButton` in `countdown`) — no hooks, no classes.

## 5. Widget quick reference (`tur:std`)

- **Layout**: `Column/Row` (flex; `mainAlignment`, `crossAlignment`,
  `mainAxisSize`; children array), `Expanded({ flex, child })` (fills
  remaining space), `Stack` + `Positioned({ left, top, right, bottom, width,
  height, child })`, `Container` (width/height/padding/color/border
  {Color,width,Radius,Position}/shadow/alignment/clipBehavior), `SizedBox`
  (whitespace), `Grid` (static; `maxCrossAxisExtent`,
  `childAspectRatio`).
- **Content**: `Text` (`spans: SpanData[]` rich text, `maxLines` +
  `overflow: "clip"|"ellipsis"|"visible"`, `selectable`, `fontWeight`),
  `Input` (+ `createTextEditingController` / `createUndoController`;
  `multiline`, `obscureText`), `Image` (`resourceId`, `fit: BoxFit`),
  `ScrollView` + `Scrollbar` (+ `createScrollController({ onScroll })`),
  `LazyList` / `LazyGrid` (+ controllers, `builder(i) => Element`,
  `itemCount`, `overscan`).
- **Interaction**: `PointerInteract` (`onClick`, `onPointerDown/Move/Up`,
  `onContextMenu`, `behavior: HitTestBehavior.Opaque|Translucent`),
  `MouseRegion` (`cursor: "pointer" | …` CSS names, `onEnter/onExit`),
  `Focusable` (`onKeyDown/onKeyUp/onFocus/onBlur`), `requestFocus(target)`.
- **Control flow**: `Condition({ condition, child, elseChild })` (thunks! —
  they re-run on branch swap, so local state inside resets),
  `Switch({ value, cases, fallback })` (same), `Each<T>({ items:
  Readable<T[]>, build })` (build re-runs for every item on any items
  change), `Fragment`, `lifecycleView(() => ({ element, onMounted$,
  beforeDestroy$ }))`, `ReadableSubscribe({ readables, onUpdate$, child })`.
- **Effects / overlay**: `Opacity({ value, child })`, `Transform({ rotate,
  scale, translateX/Y, alignment, child })`,
  `CompositedTransformTarget/Follower` + `createLayerLink()` (anchor
  linking — place the follower in a root overlay slot).
- **Animation (`tur:animation`)**: `createAnimationController`, `Tween` /
  `ColorTween`, `AnimatedContainer` / `AnimatedOpacity` /
  `AnimatedPositioned`.
- **Values**: `Color.hex("#rrggbb")` / `.rgb()` / `.rgba()`,
  `LinearGradient.create({ start, end, stops })`, enums `Axis`,
  `MainAxisAlignment`, `CrossAxisAlignment`, `MainAxisSize`,
  `HitTestBehavior`, `BoxFit`, `Alignment`, `ClipBehavior`.

## 6. Common pitfalls

**Layout**

1. **`Container` lays out only its FIRST child.** The `children` prop
   accepts an array, but the engine lays out and positions
   `children.first()` — extras are silently ignored. Multiple children →
   wrap them in a `Column`/`Row`/`Stack` first, then put that single child
   in the `Container`.
2. **Flutter-name muscle memory bites**: props are `mainAlignment` /
   `crossAlignment` (not `mainAxisAlignment` / `crossAxisAlignment`), and
   every widget takes ONE props object — `Text({ text: "hi" })`, never
   `Text("hi")`.
3. **Default cross-axis alignment of `Column`/`Row` is `Center`** (Flutter
   parity). Children won't stretch or left-align unless you pass
   `CrossAxisAlignment.Stretch` / `CrossAxisAlignment.Start`.
4. **An unbounded `Stack` leaves `Positioned` children unresolved.** Give
   the Stack a sized sibling ("sizer") or a bounded parent — see the sizer
   comment in `cases/implicit-animations`.
5. **`Expanded` only means something inside a flex parent** (`Column`/
   `Row`); elsewhere it has no remaining space to fill.
6. Use the enums, not magic numbers: `fit: BoxFit.Contain`, not `fit: 1`.

**Reactivity**

7. **In-place mutation never triggers.** Writes are equality-gated by
   reference — `ctx.get(items$).push(x)` (with or without a re-`set` of the
   same array reference) is a no-op. Always write a fresh reference:
   `ctx.set(items$, [...ctx.get(items$), item])`. Same for objects: build a
   new object, don't mutate fields.
8. **You cannot write a derived.** `set(derived$, v)` is rejected —
   computed atoms recompute from their deps. Write the source instead.
9. **`Each` items must be a `Readable<T[]>`** (a source or derive of the
   array) — a plain array literal won't track changes.
10. **A `watch` callback must not write the atom it watches** (directly or
    through a dep of a watched derived) — the engine throws "watch loop
    detected" at the offending `set`. Write results to a separate atom;
    refetch by writing a fresh trigger object.
11. **Reactive access needs a ctx — there is no `getStore()`.** Helpers
    take the ctx as a parameter; `async` bodies capture it from the
    enclosing mutation (see §4.3/§4.4).
12. **Don't think in re-renders.** View fns run exactly once (at build);
    updates are `Val<T>` prop re-reads, so define local state *inside* the
    view fn — no hooks/`useState` analogues needed. Flip side: state
    defined inside rebuildable thunks (`Each.build`, `Condition`/`Switch`
    branch thunks, lazy builders) is **re-created** (seed values) on every
    rebuild — hoist it above the boundary (enclosing view or module level).

**Modules / async**

13. **Must be `export function start()`** — `export default` is rejected
    with a "export `function start()`" load error; a missing or throwing
    `start` fails the load.
14. **No `setTimeout` / `setInterval` globals.** Use `sleep(ms).promise`
    (+ `cancel()`), and treat `isCancelError(e)` as the clean-exit branch
    of awaited loops.
15. **HTTP bodies are raw bytes.** `JSON.parse(decodeUtf8(r.body))`, never
    `JSON.parse(r.body)`.
16. **`tur:net` / `tur:filepicker` are embedder-dependent.** Statically
    importing a module the host didn't register fails the whole module
    load at the specifier. Feature-detect (`typeof request ===
    "function"`) and isolate optional imports (see the platform shims in
    `demo/playground-view/src/cases/optional-ns.*.ts`).
17. **Cleanup is for your own resources only** — timers, subscriptions,
    controllers. Tree teardown is engine-owned; there is no `unmount` to
    call, so don't try to walk/dispose elements yourself.

**Types**

18. **Always typecheck — it's the only safety net.** The `tur:` modules
    exist only as ambient TS declarations
    (`js/packages/*/src/index.d.ts`); a wrong prop name/shape is invisible
    to the bundler and at best no-ops (at worst throws) inside boa at
    runtime. Run `cd js && pnpm typecheck` (per-package `tsc --noEmit`)
    before loading a module anywhere.
19. **`Color` handles are opaque** — no arithmetic. Interpolate with
    `colorLerp(a, b, t)` or a `ColorTween`; build with
    `Color.hex/rgb/rgba` only.

## 7. Async + capabilities

Every async engine API returns **`Task<T> = { promise, cancel() }`**.
`cancel()` aborts what's abortable and **rejects the promise with a
`CancelError`** — test with `isCancelError(e)`. Await `task.promise`, never
generators; there is no `setTimeout`/`setInterval` (use `sleep(ms)`).

- **`sleep(ms)`** (tur:std) — frame-loop-precise timer.
- **`tur:net`**: `request(opts)` / `requestStream(opts)` — body is ALWAYS
  raw bytes (`ArrayBuffer` / `Uint8Array` chunks); decode with `decodeUtf8`.
  Optional module (backend-dependent).
- **`tur:clipboard`**: `clipboard.readText()` / `clipboard.writeText(text)`
  — Task-returning.
- **`tur:filepicker`**: `filePicker.pick({ accept, multiple })` →
  `PickedFile[]` (`{ name, bytes: ArrayBuffer, type, size }`),
  `filePicker.saveFile(name, bytes)`. Opt-in backend.
- Byte/string helpers: `decodeUtf8(bytes)`, `encodeUtf8(text)`.

## 8. Where to put code

- **New test/demo case**: `js/packages/tur-test-cases/cases/<name>/index.ts`
  exporting `function start()`. Split `state.ts` / `views.ts` when large
  (`todolist` pattern — module level is for state shared across views;
  single-view state lives inside the view fn). Regenerate the embedded
  sources with `node scripts/gen-cases.cjs` (run by prepare-js-fixtures).
- Case code stays **ctx-only**: reads in `derive` closures, writes in
  `mutate` closures, actions composed via `ctx.set(action, …)`.

## 9. Build / verify workflow

```sh
node scripts/prepare-js-fixtures.cjs   # install deps + build JS fixtures (before tests)
cd js && pnpm build                    # all JS packages
cd js && pnpm typecheck                # per-package tsc --noEmit (see pitfall 18)
cargo nextest run --workspace          # engine tests (per-test process isolation)
cargo clippy --workspace -- -D warnings
```

- **Engine bugs get fixed in the engine, never worked around in a case.**
  For behavior changes follow TDD: write a failing test under
  `libs/tur-integration-tests/tests/` first (red), then implement (green),
  then run the full suite + clippy.

## 10. Going deeper

- Full prop-level contracts (authoritative): `js/packages/tur-{core,std,
  animation,net,clipboard,filepicker}/src/index.d.ts` — ambient TS types.
- Engine architecture, module lifecycle internals, virtual apps, plugin/
  capability model: root `AGENTS.md`.
- Runnable examples: `js/packages/tur-test-cases/cases/` (~60 cases).
- Android on-device workflow: `.opencode/skills/android-dev/SKILL.md`.
