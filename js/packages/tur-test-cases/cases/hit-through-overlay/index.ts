import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    mount,
    mutate,
    PointerInteract,
    Positioned,
    ScrollView,
    SizedBox,
    Stack,
    Text,
    view,
} from "tur:std";

// ---------------------------------------------------------------------------
// The floating-pill pattern: a full-size invisible overlay sibling must NOT
// steal pointer/wheel input from the scrollable beneath it (Flutter: only
// things that paint or contain a hit absorb events — invisible wrappers are
// transparent to hit-testing). The pill (bottom-right anchored, opaque) is
// the only part of the overlay layer that consumes input.
//
// Counters are exposed via globalThis so the integration test can assert
// which element actually received each event.
// ---------------------------------------------------------------------------

let beneathDowns = 0;
let pillClicks = 0;

Object.assign(globalThis, {
    __getBeneathDowns: (): number => beneathDowns,
    __getPillClicks: (): number => pillClicks,
});

const App = view(() =>
    Stack({
        children: [
            // The scrollable beneath — wrapped in a gesture target so we can
            // observe pointer events reaching THROUGH the overlay.
            PointerInteract({
                onPointerDown: mutate(() => {
                    beneathDowns += 1;
                }),
                child: ScrollView({
                    queryKey: ["sv"],
                    child: Column({
                        crossAlignment: CrossAxisAlignment.Stretch,
                        children: [
                            SizedBox({ height: 300 }),
                            SizedBox({ height: 300 }),
                            SizedBox({ height: 300 }),
                        ],
                    }),
                }),
            }),
            // The invisible full-size overlay: no decoration, no gesture —
            // must be transparent to hit-testing.
            SizedBox({ width: 400, height: 600, queryKey: ["overlay"] }),
            // The floating pill, anchored bottom-right (Flutter recipe).
            Positioned({
                right: 8,
                bottom: 8,
                child: PointerInteract({
                    onClick: mutate(() => {
                        pillClicks += 1;
                    }),
                    child: Container({
                        width: 64,
                        height: 28,
                        color: Color.hex("#6366f1"),
                        queryKey: ["pill"],
                        children: [Text({ text: "to top", fontSize: 12 })],
                    }),
                }),
            }),
        ],
    }),
);

export function start() {
    mount(App);
}
