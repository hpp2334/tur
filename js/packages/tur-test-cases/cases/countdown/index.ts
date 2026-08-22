import {
    Alignment,
    Color,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    createTextEditingController,
    derive,
    type Element,
    Expanded,
    HitTestBehavior,
    Input,
    launch,
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
    sleep,
    source,
    type Task,
    Text,
    type TextController,
    type Val,
    view,
} from "tur:std";

// --- Light theme palette (slate + emerald accents) -----------------------

const COLORS = {
    pageBg: Color.hex("#f8fafc"), // slate-50
    cardBg: Color.hex("#ffffff"),
    cardBorder: Color.hex("#e2e8f0"), // slate-200
    text: Color.hex("#0f172a"), // slate-900
    textMuted: Color.hex("#64748b"), // slate-500
    textFaint: Color.hex("#94a3b8"), // slate-400
    divider: Color.hex("#e2e8f0"),
    start: Color.hex("#10b981"), // emerald-500
    startShadow: Color.rgba(16, 185, 129, 90),
    pause: Color.hex("#f59e0b"), // amber-500
    pauseShadow: Color.rgba(245, 158, 11, 90),
    urgent: Color.hex("#ef4444"), // red-500
    backdrop: Color.rgba(15, 23, 42, 110), // ~45% slate scrim
};

const DEFAULT_TIME = 60;

const remaining$ = source(DEFAULT_TIME);
const running$ = source(false);
const editing$ = source(false);
const initial$ = source(DEFAULT_TIME);
const editText$ = source("");
const editController$ = source<unknown>(null);

// Cancellable coroutine driving the 1 Hz countdown. `cancel()` halts it;
// pause/reset also flip `running$` so the loop's own check exits it.
let countdownTask: Task | null = null;
function stopCountdown() {
    countdownTask?.cancel();
    countdownTask = null;
}

const start$ = mutate((ctx, _ev: PointerInteractEvent) => {
    if (ctx.get(running$)) return;
    ctx.set(running$, true);
    stopCountdown();
    countdownTask = launch(function* () {
        while (ctx.get(running$)) {
            yield sleep(1000);
            const r = ctx.get(remaining$);
            if (r <= 1) {
                ctx.set(running$, false);
                ctx.set(remaining$, 0);
                return;
            }
            ctx.set(remaining$, r - 1);
        }
    });
});

const pause$ = mutate((ctx, _ev: PointerInteractEvent) => {
    if (!ctx.get(running$)) return;
    stopCountdown();
    ctx.set(running$, false);
});

const reset$ = mutate((ctx, _ev: PointerInteractEvent) => {
    stopCountdown();
    ctx.set(running$, false);
    ctx.set(remaining$, ctx.get(initial$));
});

const openEdit$ = mutate((ctx, _ev: PointerInteractEvent) => {
    stopCountdown();
    ctx.set(running$, false);
    ctx.set(editText$, String(ctx.get(initial$)));
    // Pre-fill the field with the current initial value so the user can
    // edit it in place rather than retyping. `initialText` is honoured at
    // controller construction time and shows up as soon as the Input
    // mounts the new controller.
    const ctrl = createTextEditingController({
        initialText: String(ctx.get(initial$)),
        onInput: mutate((ctx, text: string, _enter: boolean) =>
            ctx.set(editText$, text),
        ),
    });
    ctx.set(editController$, ctrl);
    ctx.set(editing$, true);
});

const cancelEdit$ = mutate((ctx, _ev: PointerInteractEvent) => {
    ctx.set(editing$, false);
    ctx.set(editController$, null);
});

const confirmEdit$ = mutate((ctx, _ev: PointerInteractEvent) => {
    const parsed = parseInt(ctx.get(editText$), 10);
    if (!Number.isNaN(parsed) && parsed > 0) {
        ctx.set(initial$, parsed);
        ctx.set(remaining$, parsed);
    }
    ctx.set(editing$, false);
    ctx.set(editController$, null);
});

// Format a remaining seconds count as `m:ss` (e.g. 65 -> "1:05", 5 -> "0:05").
function formatTime(totalSeconds: number): string {
    const m = Math.floor(totalSeconds / 60);
    const s = totalSeconds % 60;
    return `${m}:${String(s).padStart(2, "0")}`;
}

const isUrgent$ = derive(
    (ctx) =>
        ctx.get(running$) &&
        ctx.get(remaining$) <= 10 &&
        ctx.get(remaining$) > 0,
);

const displayColor$ = derive((ctx) =>
    ctx.get(isUrgent$) ? COLORS.urgent : COLORS.text,
);

const statusLabel$ = derive((ctx) => {
    if (ctx.get(running$)) return "Running";
    if (ctx.get(remaining$) === 0) return "Done";
    if (ctx.get(remaining$) === ctx.get(initial$)) return "Ready";
    return "Paused";
});

const statusColor$ = derive((ctx) => {
    if (ctx.get(running$)) return COLORS.start;
    if (ctx.get(remaining$) === 0) return COLORS.textFaint;
    if (ctx.get(remaining$) === ctx.get(initial$)) return COLORS.textMuted;
    return COLORS.pause;
});

// --- Reusable button helpers ---------------------------------------------

function PrimaryButton({
    label,
    bg,
    shadowColor,
    onClick,
    queryKey,
}: {
    label: Val<string>;
    bg: Color;
    shadowColor: Color;
    onClick: Mutation<[PointerInteractEvent], void>;
    queryKey?: string[];
}): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick,
            queryKey,
            child: Container({
                width: 220,
                height: 48,
                borderRadius: 12,
                color: bg,
                shadowColor,
                shadowBlur: 14,
                shadowOffset: [0, 6],
                alignment: Alignment.Center,
                children: [
                    Text({
                        text: label,
                        fontSize: 15,
                        color: Color.hex("#ffffff"),
                    }),
                ],
            }),
        }),
    });
}

function GhostButton({
    label,
    onClick,
    queryKey,
}: {
    label: string;
    onClick: Mutation<[PointerInteractEvent], void>;
    queryKey?: string[];
}): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick,
            queryKey,
            child: Container({
                padding: 10,
                borderRadius: 10,
                borderColor: COLORS.divider,
                borderWidth: 1,
                color: Color.hex("#ffffff"),
                children: [
                    Text({
                        text: label,
                        fontSize: 13,
                        color: COLORS.textMuted,
                    }),
                ],
            }),
        }),
    });
}

// --- Status pill ---------------------------------------------------------

function StatusPill(): Element {
    return Container({
        padding: 6,
        borderRadius: 999,
        borderColor: COLORS.divider,
        borderWidth: 1,
        color: Color.hex("#ffffff"),
        children: [
            Row({
                mainAxisSize: MainAxisSize.Min,
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    Container({
                        width: 7,
                        height: 7,
                        borderRadius: 999,
                        color: statusColor$,
                    }),
                    SizedBox({ width: 6 }),
                    Text({
                        text: statusLabel$,
                        fontSize: 11,
                        color: COLORS.textMuted,
                    }),
                ],
            }),
        ],
    });
}

// --- Main display --------------------------------------------------------

function TimerView(): Element {
    return Column({
        mainAxisSize: MainAxisSize.Min,
        crossAlignment: CrossAxisAlignment.Center,
        children: [
            Text({
                text: "COUNTDOWN",
                fontSize: 11,
                color: COLORS.textFaint,
            }),
            SizedBox({ height: 10 }),
            Text({
                text: derive((ctx) => formatTime(ctx.get(remaining$))),
                fontSize: 72,
                color: displayColor$,
                queryKey: ["display"],
            }),
            SizedBox({ height: 12 }),
            StatusPill(),
        ],
    });
}

function Controls(): Element {
    return Column({
        mainAxisSize: MainAxisSize.Min,
        crossAlignment: CrossAxisAlignment.Center,
        children: [
            // Primary action toggles between Start and Pause.
            Condition({
                condition: running$,
                child: () =>
                    PrimaryButton({
                        label: "Pause",
                        bg: COLORS.pause,
                        shadowColor: COLORS.pauseShadow,
                        onClick: pause$,
                        queryKey: ["btn-pause"],
                    }),
                elseChild: () =>
                    PrimaryButton({
                        label: derive((ctx) =>
                            ctx.get(remaining$) === 0 ? "Restart" : "Start",
                        ),
                        bg: COLORS.start,
                        shadowColor: COLORS.startShadow,
                        onClick: start$,
                        queryKey: ["btn-start"],
                    }),
            }),
            SizedBox({ height: 12 }),
            Row({
                mainAxisSize: MainAxisSize.Min,
                crossAlignment: CrossAxisAlignment.Center,
                children: [
                    GhostButton({
                        label: "Edit",
                        onClick: openEdit$,
                        queryKey: ["btn-edit"],
                    }),
                    SizedBox({ width: 10 }),
                    GhostButton({
                        label: "Reset",
                        onClick: reset$,
                        queryKey: ["btn-reset"],
                    }),
                ],
            }),
        ],
    });
}

// --- Edit modal ----------------------------------------------------------

function EditModal(): Element {
    // Click-anywhere-on-backdrop dismisses the modal. The inner card stops
    // propagation by being an opaque hit-test target itself.
    return Positioned({
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        child: PointerInteract({
            behavior: HitTestBehavior.Opaque,
            onClick: cancelEdit$,
            child: Container({
                color: COLORS.backdrop,
                alignment: Alignment.Center,
                children: [
                    PointerInteract({
                        behavior: HitTestBehavior.Opaque,
                        onClick: mutate((_ctx, _ev: PointerInteractEvent) => {
                            /* swallow click inside card */
                        }),
                        child: Container({
                            width: 380,
                            borderRadius: 14,
                            padding: 22,
                            color: COLORS.cardBg,
                            borderColor: COLORS.cardBorder,
                            borderWidth: 1,
                            children: [
                                Column({
                                    mainAxisSize: MainAxisSize.Min,
                                    crossAlignment: CrossAxisAlignment.Stretch,
                                    children: [
                                        Text({
                                            text: "Set duration",
                                            fontSize: 18,
                                            color: COLORS.text,
                                        }),
                                        SizedBox({ height: 6 }),
                                        Text({
                                            text: "Enter a positive integer (seconds).",
                                            fontSize: 13,
                                            color: COLORS.textMuted,
                                        }),
                                        SizedBox({ height: 16 }),
                                        Container({
                                            padding: 4,
                                            borderRadius: 8,
                                            borderColor: COLORS.divider,
                                            borderWidth: 1,
                                            queryKey: ["edit-input"],
                                            children: [
                                                Input({
                                                    controller: derive((ctx) =>
                                                        ctx.get(
                                                            editController$,
                                                        ),
                                                    ) as unknown as TextController,
                                                    placeholder:
                                                        "Positive integer",
                                                    fontSize: 14,
                                                    width: 332,
                                                    height: 32,
                                                }),
                                            ],
                                        }),
                                        SizedBox({ height: 18 }),
                                        Row({
                                            mainAlignment:
                                                MainAxisAlignment.End,
                                            mainAxisSize: MainAxisSize.Min,
                                            children: [
                                                GhostButton({
                                                    label: "Cancel",
                                                    onClick: cancelEdit$,
                                                }),
                                                SizedBox({ width: 8 }),
                                                PrimaryButton({
                                                    label: "Save",
                                                    bg: COLORS.start,
                                                    shadowColor:
                                                        COLORS.startShadow,
                                                    onClick: confirmEdit$,
                                                    queryKey: ["btn-confirm"],
                                                }),
                                            ],
                                        }),
                                    ],
                                }),
                            ],
                        }),
                    }),
                ],
            }),
        }),
    });
}

// --- Page ----------------------------------------------------------------

export default view(() =>
    Expanded({
        child: Stack({
            children: [
                Container({
                    color: COLORS.pageBg,
                    alignment: Alignment.Center,
                    children: [
                        Column({
                            mainAlignment: MainAxisAlignment.Center,
                            crossAlignment: CrossAxisAlignment.Center,
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                TimerView(),
                                SizedBox({ height: 36 }),
                                Controls(),
                            ],
                        }),
                    ],
                }),
                Condition({
                    condition: editing$,
                    child: () => EditModal(),
                }),
            ],
        }),
    }),
);
