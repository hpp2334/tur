import {
    Color,
    Container,
    derive,
    get,
    MainAxisAlignment,
    Row,
    set,
    source,
    view,
} from "builtin:tur/core";

// Reactive width of the outer container. Exposed to Rust tests via globalThis
// so the test can change it without a click (a click would mark extra nodes
// dirty via the gesture handler and mask the position-invalidation bug).
const width$ = source(100);

Object.assign(globalThis, {
    __setWidth: (w: number): void => {
        set(width$, w);
    },
});

// Reproduces the divider-drag position gap.
//
//   Container(width = width$)     <- reads the reactive source; marked dirty
//     └ Row(mainAlignment=End)    <- intermediate descendant: NOT marked dirty,
//                                    but re-measured via `constraints_changed`
//                                    (its width tracks `width$`). Its
//                                    `perform_layout` pushes the child to the
//                                    trailing edge, so the child's X offset
//                                    = width$ - 20.
//       └ Container(width=20)     <- "tracker": position depends on the Row's
//                                    width, hence on `width$`.
//
// When `width$` changes, only the outer container (and its ancestors) are
// marked dirty (`mark_dirty` walks up only). The Row re-runs its layout
// pass because its constraints changed. Under the current merged single-pass
// `perform_layout` this repositions correctly by construction — but it once
// regressed under a split size/position design where the position phase could
// skip a constraint-driven descendant, leaving the tracker at a stale X offset
// (playground symptom: the editor scrollbar stayed painted at its old
// position after a divider drag).
export default view(() =>
    Container({
        width: derive(() => get(width$)),
        children: [
            Row({
                mainAlignment: MainAxisAlignment.End,
                children: [
                    Container({
                        width: 20,
                        height: 20,
                        color: Color.hex("#22c55e"),
                        queryKey: ["tracker"],
                    }),
                ],
            }),
        ],
    }),
);
