import {
    AnimatedContainer,
    AnimatedOpacity,
    AnimatedPositioned,
} from "tur:animation";
import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    type Element,
    Expanded,
    MainAxisAlignment,
    MainAxisSize,
    MouseRegion,
    type Mutation,
    mount,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    type Readable,
    Row,
    SizedBox,
    Stack,
    source,
    Text,
    view,
} from "tur:std";

// ---------------------------------------------------------------------------
// "Implicit Animations" — demonstrates tur's AnimatedContainer /
// AnimatedOpacity / AnimatedPositioned (Flutter's ImplicitlyAnimatedWidget
// family). No AnimationController, no source+derive+lerp boilerplate: pass a
// target value + duration + curve and the element animates from its previous
// value automatically when the target changes.
//
// Click "Toggle" to flip between two states. All three animated elements
// retarget together:
//   - AnimatedContainer:  borderRadius 12 ↔ 40, color swap, shadowBlur 16 ↔ 32
//   - AnimatedOpacity:    label alpha 0.45 ↔ 1.0
//   - AnimatedPositioned: card slides left ↔ right inside the Stack
// ---------------------------------------------------------------------------

const DURATION = 600;

// The factories take the local state as parameters — the view fn owns it (it
// runs exactly once at build, so the atoms are stable for the tree's life).

function Card(expanded$: Readable<boolean>): Element {
    return Stack({
        children: [
            // Sizer: gives the inner Stack a finite canvas so Positioned
            // children resolve against known bounds.
            Container({ width: 340, height: 220 }),
            AnimatedPositioned({
                left: derive((ctx) => (ctx.get(expanded$) ? 160 : 30)),
                top: 30,
                duration: DURATION,
                curve: "easeInOut",
                child: AnimatedOpacity({
                    value: derive((ctx) => (ctx.get(expanded$) ? 1.0 : 0.45)),
                    duration: DURATION,
                    curve: "easeInOut",
                    child: AnimatedContainer({
                        width: 150,
                        height: 160,
                        borderRadius: derive((ctx) =>
                            ctx.get(expanded$) ? 40 : 12,
                        ),
                        color: derive((ctx) =>
                            ctx.get(expanded$)
                                ? Color.rgb(99, 102, 241)
                                : Color.rgb(14, 165, 233),
                        ),
                        shadowColor: Color.rgba(15, 23, 42, 120),
                        shadowBlur: derive((ctx) =>
                            ctx.get(expanded$) ? 32 : 16,
                        ),
                        shadowOffset: [0, 8],
                        duration: DURATION,
                        curve: "easeInOut",
                        alignment: Alignment.Center,
                        children: [
                            Text({
                                text: derive((ctx) =>
                                    ctx.get(expanded$) ? "Expanded" : "Compact",
                                ),
                                fontSize: 18,
                                color: Color.rgb(255, 255, 255),
                            }),
                        ],
                    }),
                }),
            }),
        ],
    });
}

function ToggleButton(
    expanded$: Readable<boolean>,
    toggle: Mutation<[], void>,
): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: toggle as unknown as Mutation<
                [PointerInteractEvent],
                void
            >,
            child: Container({
                padding: 10,
                borderRadius: 8,
                color: Color.rgb(30, 41, 59),
                children: [
                    Text({
                        text: derive((ctx) =>
                            ctx.get(expanded$) ? "◀ Compact" : "Expand ▶",
                        ),
                        fontSize: 12,
                        color: Color.rgb(226, 232, 240),
                    }),
                ],
            }),
        }),
    });
}

const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so the atom and
    // mutation are stable for the life of the tree — no need to hoist them to
    // module level.
    const expanded$ = source(false);
    const toggle = mutate((ctx) => ctx.set(expanded$, !ctx.get(expanded$)));

    return Expanded({
        child: Container({
            color: Color.rgb(248, 250, 252),
            children: [
                Column({
                    mainAlignment: MainAxisAlignment.Center,
                    crossAlignment: CrossAxisAlignment.Center,
                    mainAxisSize: MainAxisSize.Min,
                    children: [
                        Text({
                            text: "Implicit Animations",
                            fontSize: 16,
                            color: Color.rgb(15, 23, 42),
                        }),
                        SizedBox({ height: 4 }),
                        Text({
                            text: "AnimatedContainer · AnimatedOpacity · AnimatedPositioned",
                            fontSize: 10,
                            color: Color.rgb(100, 116, 139),
                        }),
                        SizedBox({ height: 24 }),
                        Card(expanded$),
                        SizedBox({ height: 24 }),
                        Row({
                            mainAxisSize: MainAxisSize.Min,
                            children: [ToggleButton(expanded$, toggle)],
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
