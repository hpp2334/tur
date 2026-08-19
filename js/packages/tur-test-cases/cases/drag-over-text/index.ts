import {
    Color,
    Container,
    createStore,
    mutate,
    PointerInteract,
    Text,
    view,
} from "tur:std";

export const store = createStore();

// ---------------------------------------------------------------------------
// Regression fixture for the touch-drag-stealing bug.
//
// A `PointerInteract` wraps a `Container` whose child is a non-selectable
// `Text` — exactly the jigsaw-piece structure (`Positioned → Transform →
// PointerInteract → Container → Text`). The Text paints ON TOP of the
// PointerInteract, so it is the first element in the touch hit-path.
//
// On **mouse** drags the PointerInteract fires onPointerDown/onPointerMove
// (the mouse path dispatches to the whole hit-path, no claim probe). On
// **touch** drags the gesture arena probes the hit-path top-down and the
// first gesture-capable element that returns `true` wins — so a Text that
// claims gestures (the bug) steals the drag and the PointerInteract never
// sees onPointerDown.
//
// Counters are exposed via globalThis so the integration test can assert
// whether onPointerDown actually fired.
// ---------------------------------------------------------------------------

let downCount = 0;
let moveCount = 0;

Object.assign(globalThis, {
    __getDownCount: (): number => downCount,
    __getMoveCount: (): number => moveCount,
    __resetDrag: (): void => {
        downCount = 0;
        moveCount = 0;
    },
});

export default view(() =>
    PointerInteract({
        onPointerDown: mutate(() => {
            downCount += 1;
        }),
        onPointerMove: mutate(() => {
            moveCount += 1;
        }),
        child: Container({
            width: 200,
            height: 200,
            color: Color.hex("#6366f1"),
            queryKey: ["drag-target"],
            children: [Text({ text: "drag me" })],
        }),
    }),
);
