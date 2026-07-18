import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    type Element,
    Expanded,
    get,
    MainAxisAlignment,
    MainAxisSize,
    MouseRegion,
    type Mutation,
    mutate,
    PointerInteract,
    type PointerInteractEvent,
    Positioned,
    Row,
    SizedBox,
    Stack,
    set,
    source,
    Text,
    view,
} from "builtin:tur/std";
import {
    type AnimationController,
    createAnimationController,
    ColorTween,
    Transform,
    Tween,
} from "builtin:tur/animation";

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
        onTick: mutate((_ctx, v: number) => {
            set(progress$, v);
        }),
        onEnd: mutate(() => {
            set(status$, ctrl.status);
        }),
    });
}

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
// Mutations
// ---------------------------------------------------------------------------

const playForward = mutate(() => {
    ctrl.forward();
    set(status$, ctrl.status);
});
const playReverse = mutate(() => {
    ctrl.reverse();
    set(status$, ctrl.status);
});
const pause = mutate(() => {
    ctrl.pause();
    set(status$, ctrl.status);
});
const resume = mutate(() => {
    ctrl.resume();
    set(status$, ctrl.status);
});
const stop = mutate(() => {
    ctrl.stop();
    set(status$, ctrl.status);
    set(progress$, ctrl.value);
});
const setSpeed = mutate((_ctx, factor: number, label: string) => {
    ctrl.setSpeed(factor);
    set(speedLabel$, label);
});
const setCurve = mutate(
    (_ctx, curve: "linear" | "easeIn" | "easeOut" | "easeInOut") => {
        const t = get(progress$);
        ctrl = createController(curve, get(looping$));
        ctrl.seek(t);
        set(curveLabel$, curve);
        set(status$, ctrl.status);
    },
);
const toggleLooping = mutate(() => {
    const next = !get(looping$);
    const t = get(progress$);
    ctrl = createController(get(curveLabel$), next);
    ctrl.seek(t);
    set(looping$, next);
    set(status$, ctrl.status);
});

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

function Card(): Element {
    // Card animates width, borderRadius, hue based on progress$ via the
    // Tween / ColorTween abstractions (Flutter-aligned). The explicit
    // AnimationController drives `progress$` continuously; the tweens handle
    // the value interpolation that previously needed hand-rolled `lerp`.
    return Container({
        // Width: 120 → 280
        width: derive(() => widthTween.lerp(get(progress$))),
        height: 160,
        borderRadius: derive(() => radiusTween.lerp(get(progress$))),
        color: derive(() => colorTween.lerp(get(progress$))),
        shadowColor: Color.rgba(15, 23, 42, 80),
        shadowBlur: 24,
        shadowOffset: [0, 8],
        alignment: Alignment.Center,
        children: [
            // Rotating inner shape — demonstrates the Transform element.
            Transform({
                rotate: derive(() => get(progress$) * 2 * Math.PI),
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

function OrbitingDot(): Element {
    // A dot that orbits around the card center.
    return Positioned({
        left: derive(() => 140 + 80 * Math.cos(2 * Math.PI * get(progress$))),
        top: derive(() => 80 + 80 * Math.sin(2 * Math.PI * get(progress$))),
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

function ProgressReadout(): Element {
    return Text({
        text: derive(() => `${Math.round(get(progress$) * 100)}%`),
        fontSize: 12,
        color: Color.rgba(71, 85, 105, 255),
    });
}

function StatusBadge(): Element {
    return Container({
        padding: 6,
        borderRadius: 999,
        color: derive(() => {
            const s = get(status$);
            if (s === "forward" || s === "reverse") {
                return Color.rgba(34, 197, 94, 255);
            }
            if (s === "paused") return Color.rgba(245, 158, 11, 255);
            if (s === "completed") return Color.rgba(99, 102, 241, 255);
            return Color.rgba(148, 163, 184, 255);
        }),
        children: [
            Text({
                text: derive(() => get(status$).toUpperCase()),
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

function SpeedButton(factor: number, label: string): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate(() =>
                set(setSpeed, factor, label),
            ) as unknown as Mutation<[PointerInteractEvent], void>,
            child: Container({
                padding: 6,
                borderRadius: 6,
                color: derive(() =>
                    get(speedLabel$) === label
                        ? Color.hex("#1e293b")
                        : Color.hex("#e2e8f0"),
                ),
                children: [
                    Text({
                        text: label,
                        fontSize: 10,
                        color: derive(() =>
                            get(speedLabel$) === label
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
): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate(() => set(setCurve, curve)) as unknown as Mutation<
                [PointerInteractEvent],
                void
            >,
            child: Container({
                padding: 6,
                borderRadius: 6,
                color: derive(() =>
                    get(curveLabel$) === curve
                        ? Color.hex("#0d9488")
                        : Color.hex("#e2e8f0"),
                ),
                children: [
                    Text({
                        text: label,
                        fontSize: 10,
                        color: derive(() =>
                            get(curveLabel$) === curve
                                ? Color.hex("#ffffff")
                                : Color.hex("#475569"),
                        ),
                    }),
                ],
            }),
        }),
    });
}

function LoopButton(): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate(() => {
                set(toggleLooping);
            }) as unknown as Mutation<[PointerInteractEvent], void>,
            child: Container({
                padding: 6,
                borderRadius: 6,
                color: derive(() =>
                    get(looping$) ? Color.hex("#db2777") : Color.hex("#e2e8f0"),
                ),
                children: [
                    Text({
                        text: derive(() => (get(looping$) ? "Loop ✓" : "Loop")),
                        fontSize: 10,
                        color: derive(() =>
                            get(looping$)
                                ? Color.hex("#ffffff")
                                : Color.hex("#475569"),
                        ),
                    }),
                ],
            }),
        }),
    });
}

export default view(() =>
    Expanded({
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
                                StatusBadge(),
                                SizedBox({ width: 12 }),
                                ProgressReadout(),
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
                                    child: Card(),
                                }),
                                OrbitingDot(),
                            ],
                        }),

                        SizedBox({ height: 32 }),

                        // Transport controls
                        Row({
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                Button("Play", playForward, "#22c55e"),
                                SizedBox({ width: 6 }),
                                Button("Pause", pause, "#f59e0b"),
                                SizedBox({ width: 6 }),
                                Button("Resume", resume, "#0ea5e9"),
                                SizedBox({ width: 6 }),
                                Button("Reverse", playReverse, "#8b5cf6"),
                                SizedBox({ width: 6 }),
                                Button("Stop", stop, "#ef4444"),
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
                                SpeedButton(0.5, "0.5x"),
                                SizedBox({ width: 4 }),
                                SpeedButton(1, "1x"),
                                SizedBox({ width: 4 }),
                                SpeedButton(2, "2x"),
                                SizedBox({ width: 4 }),
                                SpeedButton(4, "4x"),
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
                                CurveButton("linear", "linear"),
                                SizedBox({ width: 4 }),
                                CurveButton("easeIn", "easeIn"),
                                SizedBox({ width: 4 }),
                                CurveButton("easeOut", "easeOut"),
                                SizedBox({ width: 4 }),
                                CurveButton("easeInOut", "easeInOut"),
                                SizedBox({ width: 12 }),
                                LoopButton(),
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
    }),
);
