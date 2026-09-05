import {
    type AnimationController,
    ColorTween,
    createAnimationController,
    Tween,
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
    Positioned,
    Row,
    SizedBox,
    Stack,
    source,
    Text,
    Transform,
    view,
} from "tur:std";

// ---------------------------------------------------------------------------
// "Animated Card Studio" — a demo of tur's animation API.
//
// A centered card animates:
//   - width: 120 → 280
//   - borderRadius: 8 → 40
//   - hue: indigo → coral (via per-tick Color.rgb)
//   - rotation: 0 → 2π (looping)
//
// Controls:
//   - Play / Pause / Resume / Reverse / Stop
//   - Speed selector (0.5x / 1x / 2x / 4x)
//   - Curve dropdown (linear / easeIn / easeOut / easeInOut) — recreates controller
// ---------------------------------------------------------------------------

// --- State ------------------------------------------------------------------
// Created INSIDE the root view fn below: view functions run exactly once,
// when the element tree is built (at `mount`) — prop updates never re-invoke
// them — so this cluster is stable local state, not module-level state.
// Bundled into a factory so the widget builders further down can take it as
// a parameter.

function createAnimState() {
    const progress$ = source(0); // 0..1 raw progress
    const status$ = source<
        "stopped" | "forward" | "reverse" | "completed" | "paused"
    >("stopped");
    const speedLabel$ = source("1x");
    const curveLabel$ = source<"linear" | "easeIn" | "easeOut" | "easeInOut">(
        "easeInOut",
    );
    const looping$ = source(false);

    // Mutable controller holder. Recreated when the curve changes or when
    // looping is toggled; in-place mutated for speed/seek.
    let ctrl: AnimationController = createController("easeInOut", false);

    function createController(
        curve: "linear" | "easeIn" | "easeOut" | "easeInOut",
        looping: boolean,
    ): AnimationController {
        return createAnimationController({
            duration: 2400,
            curve,
            repeat: looping ? "infinite" : 1,
            onTick: mutate((ctx, v: number) => {
                ctx.set(progress$, v);
            }),
            onEnd: mutate((ctx) => {
                ctx.set(status$, ctrl.status);
            }),
        });
    }

    const playForward = mutate((ctx) => {
        ctrl.forward();
        ctx.set(status$, ctrl.status);
    });
    const playReverse = mutate((ctx) => {
        ctrl.reverse();
        ctx.set(status$, ctrl.status);
    });
    const pause = mutate((ctx) => {
        ctrl.pause();
        ctx.set(status$, ctrl.status);
    });
    const resume = mutate((ctx) => {
        ctrl.resume();
        ctx.set(status$, ctrl.status);
    });
    const stop = mutate((ctx) => {
        ctrl.stop();
        ctx.set(status$, ctrl.status);
        ctx.set(progress$, ctrl.value);
    });
    const setSpeed = mutate((ctx, factor: number, label: string) => {
        ctrl.setSpeed(factor);
        ctx.set(speedLabel$, label);
    });
    const setCurve = mutate(
        (ctx, curve: "linear" | "easeIn" | "easeOut" | "easeInOut") => {
            const t = ctx.get(progress$);
            ctrl = createController(curve, ctx.get(looping$));
            ctrl.seek(t);
            ctx.set(curveLabel$, curve);
            ctx.set(status$, ctrl.status);
        },
    );
    const toggleLooping = mutate((ctx) => {
        const next = !ctx.get(looping$);
        const t = ctx.get(progress$);
        ctrl = createController(ctx.get(curveLabel$), next);
        ctrl.seek(t);
        ctx.set(looping$, next);
        ctx.set(status$, ctrl.status);
    });

    return {
        progress$,
        status$,
        speedLabel$,
        curveLabel$,
        looping$,
        playForward,
        playReverse,
        pause,
        resume,
        stop,
        setSpeed,
        setCurve,
        toggleLooping,
    };
}

type AnimState = ReturnType<typeof createAnimState>;

// Color interpolation: indigo (99,102,241) → coral (232,93,68). The new
// `ColorTween` (tur's Flutter-aligned Tween) replaces the hand-rolled
// per-channel `lerp` below — same component-wise interpolation, no manual
// rounding math in user code.
const widthTween = Tween({ begin: 120, end: 280 });
const radiusTween = Tween({ begin: 8, end: 40 });
const colorTween = ColorTween({
    begin: Color.rgba(99, 102, 241, 255),
    end: Color.rgba(232, 93, 68, 255),
});

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

function Card(s: AnimState): Element {
    // Card animates width, borderRadius, hue based on progress$ via the
    // Tween / ColorTween abstractions (Flutter-aligned). The explicit
    // AnimationController drives `progress$` continuously; the tweens handle
    // the value interpolation that previously needed hand-rolled `lerp`.
    return Container({
        // Width: 120 → 280
        width: derive((ctx) => widthTween.lerp(ctx.get(s.progress$))),
        height: 160,
        borderRadius: derive((ctx) => radiusTween.lerp(ctx.get(s.progress$))),
        color: derive((ctx) => colorTween.lerp(ctx.get(s.progress$))),
        shadowColor: Color.rgba(15, 23, 42, 80),
        shadowBlur: 24,
        shadowOffset: [0, 8],
        alignment: Alignment.Center,
        children: [
            // Rotating inner shape — demonstrates the Transform element.
            Transform({
                rotate: derive((ctx) => ctx.get(s.progress$) * 2 * Math.PI),
                child: Container({
                    width: 60,
                    height: 60,
                    borderRadius: 12,
                    color: Color.rgba(255, 255, 255, 255),
                }),
            }),
        ],
    });
}

function OrbitingDot(s: AnimState): Element {
    // A dot that orbits around the card center.
    return Positioned({
        left: derive(
            (ctx) => 140 + 80 * Math.cos(2 * Math.PI * ctx.get(s.progress$)),
        ),
        top: derive(
            (ctx) => 80 + 80 * Math.sin(2 * Math.PI * ctx.get(s.progress$)),
        ),
        child: Container({
            width: 20,
            height: 20,
            borderRadius: 999,
            color: Color.rgba(34, 197, 94, 255),
            shadowColor: Color.rgba(34, 197, 94, 120),
            shadowBlur: 16,
            shadowOffset: [0, 0],
        }),
    });
}

function ProgressReadout(s: AnimState): Element {
    return Text({
        text: derive((ctx) => `${Math.round(ctx.get(s.progress$) * 100)}%`),
        fontSize: 12,
        color: Color.rgba(71, 85, 105, 255),
    });
}

function StatusBadge(s: AnimState): Element {
    return Container({
        padding: 6,
        borderRadius: 999,
        color: derive((ctx) => {
            const s2 = ctx.get(s.status$);
            if (s2 === "forward" || s2 === "reverse") {
                return Color.rgba(34, 197, 94, 255);
            }
            if (s2 === "paused") return Color.rgba(245, 159, 11, 255);
            if (s2 === "completed") return Color.rgba(99, 102, 241, 255);
            return Color.rgba(148, 163, 184, 255);
        }),
        children: [
            Text({
                text: derive((ctx) => ctx.get(s.status$).toUpperCase()),
                fontSize: 10,
                color: Color.rgba(255, 255, 255, 255),
            }),
        ],
    });
}

function Button(
    label: string,
    onClick: Mutation<[], void>,
    color = "#4f46e5",
): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: onClick as unknown as Mutation<
                [PointerInteractEvent],
                void
            >,
            child: Container({
                padding: 8,
                borderRadius: 6,
                color: Color.hex(color),
                children: [
                    Text({
                        text: label,
                        fontSize: 11,
                        color: Color.hex("#ffffff"),
                    }),
                ],
            }),
        }),
    });
}

function SpeedButton(factor: number, label: string, s: AnimState): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx) =>
                ctx.set(s.setSpeed, factor, label),
            ) as unknown as Mutation<[PointerInteractEvent], void>,
            child: Container({
                padding: 6,
                borderRadius: 6,
                color: derive((ctx) =>
                    ctx.get(s.speedLabel$) === label
                        ? Color.hex("#1e293b")
                        : Color.hex("#e2e8f0"),
                ),
                children: [
                    Text({
                        text: label,
                        fontSize: 10,
                        color: derive((ctx) =>
                            ctx.get(s.speedLabel$) === label
                                ? Color.hex("#ffffff")
                                : Color.hex("#475569"),
                        ),
                    }),
                ],
            }),
        }),
    });
}

function CurveButton(
    curve: "linear" | "easeIn" | "easeOut" | "easeInOut",
    label: string,
    s: AnimState,
): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx) =>
                ctx.set(s.setCurve, curve),
            ) as unknown as Mutation<[PointerInteractEvent], void>,
            child: Container({
                padding: 6,
                borderRadius: 6,
                color: derive((ctx) =>
                    ctx.get(s.curveLabel$) === curve
                        ? Color.hex("#0d9488")
                        : Color.hex("#e2e8f0"),
                ),
                children: [
                    Text({
                        text: label,
                        fontSize: 10,
                        color: derive((ctx) =>
                            ctx.get(s.curveLabel$) === curve
                                ? Color.hex("#ffffff")
                                : Color.hex("#475569"),
                        ),
                    }),
                ],
            }),
        }),
    });
}

function LoopButton(s: AnimState): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx) => {
                ctx.set(s.toggleLooping);
            }) as unknown as Mutation<[PointerInteractEvent], void>,
            child: Container({
                padding: 6,
                borderRadius: 6,
                color: derive((ctx) =>
                    ctx.get(s.looping$)
                        ? Color.hex("#db2777")
                        : Color.hex("#e2e8f0"),
                ),
                children: [
                    Text({
                        text: derive((ctx) =>
                            ctx.get(s.looping$) ? "Loop ✓" : "Loop",
                        ),
                        fontSize: 10,
                        color: derive((ctx) =>
                            ctx.get(s.looping$)
                                ? Color.hex("#ffffff")
                                : Color.hex("#475569"),
                        ),
                    }),
                ],
            }),
        }),
    });
}

const App = view(() => {
    // Local state: the view fn runs exactly once (at build), so the whole
    // animation state cluster is stable for the life of the tree.
    const s = createAnimState();

    return Expanded({
        child: Container({
            color: Color.hex("#f8fafc"),
            children: [
                Column({
                    mainAlignment: MainAxisAlignment.Center,
                    crossAlignment: CrossAxisAlignment.Center,
                    mainAxisSize: MainAxisSize.Min,
                    children: [
                        Text({
                            text: "Animated Card Studio",
                            fontSize: 16,
                            color: Color.hex("#0f172a"),
                        }),
                        SizedBox({ height: 4 }),
                        Row({
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                StatusBadge(s),
                                SizedBox({ width: 12 }),
                                ProgressReadout(s),
                            ],
                        }),
                        SizedBox({ height: 32 }),

                        // Animated card + orbiting dot
                        Stack({
                            children: [
                                Container({
                                    width: 360,
                                    height: 200,
                                }),
                                Positioned({
                                    left: 100,
                                    top: 20,
                                    child: Card(s),
                                }),
                                OrbitingDot(s),
                            ],
                        }),

                        SizedBox({ height: 32 }),

                        // Transport controls
                        Row({
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                Button("Play", s.playForward, "#22c55e"),
                                SizedBox({ width: 6 }),
                                Button("Pause", s.pause, "#f59e0b"),
                                SizedBox({ width: 6 }),
                                Button("Resume", s.resume, "#0ea5e9"),
                                SizedBox({ width: 6 }),
                                Button("Reverse", s.playReverse, "#8b5cf6"),
                                SizedBox({ width: 6 }),
                                Button("Stop", s.stop, "#ef4444"),
                            ],
                        }),

                        SizedBox({ height: 16 }),

                        // Speed selector
                        Row({
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                Text({
                                    text: "Speed:",
                                    fontSize: 11,
                                    color: Color.hex("#64748b"),
                                }),
                                SizedBox({ width: 8 }),
                                SpeedButton(0.5, "0.5x", s),
                                SizedBox({ width: 4 }),
                                SpeedButton(1, "1x", s),
                                SizedBox({ width: 4 }),
                                SpeedButton(2, "2x", s),
                                SizedBox({ width: 4 }),
                                SpeedButton(4, "4x", s),
                            ],
                        }),

                        SizedBox({ height: 12 }),

                        // Curve selector
                        Row({
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                Text({
                                    text: "Curve:",
                                    fontSize: 11,
                                    color: Color.hex("#64748b"),
                                }),
                                SizedBox({ width: 8 }),
                                CurveButton("linear", "linear", s),
                                SizedBox({ width: 4 }),
                                CurveButton("easeIn", "easeIn", s),
                                SizedBox({ width: 4 }),
                                CurveButton("easeOut", "easeOut", s),
                                SizedBox({ width: 4 }),
                                CurveButton("easeInOut", "easeInOut", s),
                                SizedBox({ width: 12 }),
                                LoopButton(s),
                            ],
                        }),

                        SizedBox({ height: 16 }),

                        Text({
                            text: "Controls: pause mid-play, change speed/curve, resume",
                            fontSize: 10,
                            color: Color.hex("#94a3b8"),
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
