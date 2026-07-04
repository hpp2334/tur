import {
    Alignment,
    type Brush,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    derive,
    type Element,
    Expanded,
    get,
    HitTestBehavior,
    ImageEdgy,
    InputEdgy,
    MainAxisAlignment,
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    Row,
    SizedBox,
    type StoreCtx,
    Text,
} from "builtin:tur/core";
import {
    closeAddModal,
    closeRemoveModal,
    confirmRemove,
    descCtrl,
    getIcon,
    removeTarget$,
    requestRemove,
    submitAdd,
    type Task,
    tasks$,
    titleCtrl,
    toggleTask,
} from "./state";

// --- Light palette (Notion / Linear-style) --------------------------------

const COLORS = {
    cardBg: Color.hex("#ffffff"),
    cardBorder: Color.hex("#e2e8f0"), // slate-200
    text: Color.hex("#0f172a"), // slate-900
    textMuted: Color.hex("#64748b"), // slate-500
    textStrike: Color.hex("#94a3b8"), // slate-400
    accent: Color.hex("#4f46e5"), // indigo-600
    accentFg: Color.hex("#ffffff"),
    success: Color.hex("#22c55e"), // green-500
    successBorder: Color.hex("#16a34a"), // green-600
    danger: Color.hex("#ef4444"), // red-500
    dangerFg: Color.hex("#ffffff"),
    inputBg: Color.hex("#f8fafc"), // slate-50
    inputBorder: Color.hex("#cbd5e1"), // slate-300
    backdrop: Color.rgba(15, 23, 42, 110), // ~45% slate scrim
    subtleButton: Color.hex("#f1f5f9"), // slate-100
    subtleButtonFg: Color.hex("#334155"), // slate-700
    cardShadow: Color.rgba(15, 23, 42, 8),
    cardShadowLg: Color.rgba(15, 23, 42, 24),
} as const;

// --- TaskItem -------------------------------------------------------------

export function TaskItem({
    task,
    index,
}: {
    task: Task;
    index: number;
}): Element {
    return Container({
        borderRadius: 10,
        padding: 14,
        color: COLORS.cardBg,
        borderColor: COLORS.cardBorder,
        borderWidth: 1,
        shadowColor: COLORS.cardShadow,
        shadowBlur: 6,
        shadowOffset: [0, 1],
        children: [
            Row({
                crossAlignment: CrossAxisAlignment.Start,
                children: [
                    // Checkbox — toggles completion.
                    MouseRegion({
                        cursor: "pointer",
                        child: PointerInteract({
                            onClick: mutate((ctx, _ev) =>
                                toggleTask(ctx, index),
                            ),
                            child: Container({
                                width: 20,
                                height: 20,
                                borderRadius: 6,
                                color:
                                    (task.completed
                                        ? COLORS.success
                                        : null) as unknown as Brush,
                                borderColor: task.completed
                                    ? COLORS.successBorder
                                    : COLORS.inputBorder,
                                borderWidth: task.completed ? 0 : 1.5,
                                alignment: Alignment.Center,
                                children: [
                                    task.completed
                                        ? ImageEdgy({
                                              resourceId: getIcon("check"),
                                              width: 13,
                                              height: 13,
                                              queryKey: ["check-icon"],
                                          })
                                        : SizedBox({ width: 0, height: 0 }),
                                ],
                            }),
                        }),
                    }),
                    SizedBox({ width: 12 }),
                    // Title + description.
                    Expanded({
                        child: Column({
                            crossAlignment: CrossAxisAlignment.Start,
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                Text({
                                    text: task.title,
                                    fontSize: 14,
                                    color: task.completed
                                        ? COLORS.textStrike
                                        : COLORS.text,
                                }),
                                task.description
                                    ? SizedBox({ height: 3 })
                                    : SizedBox({ width: 0, height: 0 }),
                                task.description
                                    ? Text({
                                          text: task.description,
                                          fontSize: 12,
                                          color: COLORS.textMuted,
                                      })
                                    : SizedBox({ width: 0, height: 0 }),
                            ],
                        }),
                    }),
                    SizedBox({ width: 8 }),
                    // Delete button.
                    MouseRegion({
                        cursor: "pointer",
                        child: PointerInteract({
                            onClick: mutate((ctx, _ev) =>
                                requestRemove(ctx, index),
                            ),
                            child: Container({
                                width: 26,
                                height: 26,
                                borderRadius: 6,
                                color: COLORS.subtleButton,
                                alignment: Alignment.Center,
                                children: [
                                    ImageEdgy({
                                        resourceId: getIcon("close"),
                                        width: 13,
                                        height: 13,
                                        queryKey: ["close-icon"],
                                    }),
                                ],
                            }),
                        }),
                    }),
                ],
            }),
        ],
    });
}

// --- Modal shell (backdrop + centered card with click-to-dismiss) ----------

function ModalShell({
    onBackdropClick,
    card,
}: {
    onBackdropClick: (ctx: StoreCtx) => void;
    card: Element;
}): Element {
    return PointerInteract({
        behavior: HitTestBehavior.Opaque,
        onClick: mutate((ctx, _ev) => onBackdropClick(ctx)),
        child: Container({
            color: COLORS.backdrop,
            alignment: Alignment.Center,
            children: [
                // Card wrapper — opaque so backdrop dismissal doesn't fire
                // when the user clicks inside the card.
                PointerInteract({
                    behavior: HitTestBehavior.Opaque,
                    onClick: mutate((_ctx, _ev) => {
                        /* swallow click */
                    }),
                    child: card,
                }),
            ],
        }),
    });
}

function Button({
    label,
    bg,
    fg,
    onClick,
}: {
    label: string;
    bg: Color;
    fg: Color;
    onClick: (ctx: StoreCtx) => void;
}): Element {
    return MouseRegion({
        cursor: "pointer",
        child: PointerInteract({
            onClick: mutate((ctx, _ev) => onClick(ctx)),
            child: Container({
                padding: 9,
                borderRadius: 7,
                color: bg,
                children: [
                    Text({
                        text: label,
                        fontSize: 13,
                        color: fg,
                    }),
                ],
            }),
        }),
    });
}

// --- AddTaskModal ---------------------------------------------------------

export function AddTaskModal(): Element {
    return ModalShell({
        onBackdropClick: closeAddModal,
        card: Container({
            width: 380,
            borderRadius: 14,
            padding: 22,
            color: COLORS.cardBg,
            borderColor: COLORS.cardBorder,
            borderWidth: 1,
            shadowColor: COLORS.cardShadowLg,
            shadowBlur: 30,
            shadowOffset: [0, 12],
            children: [
                Column({
                    crossAlignment: CrossAxisAlignment.Stretch,
                    mainAxisSize: MainAxisSize.Min,
                    children: [
                        Text({
                            text: "New Task",
                            fontSize: 17,
                            color: COLORS.text,
                        }),
                        SizedBox({ height: 16 }),
                        Text({
                            text: "Title",
                            fontSize: 11,
                            color: COLORS.textMuted,
                        }),
                        SizedBox({ height: 6 }),
                        Container({
                            borderRadius: 7,
                            padding: 9,
                            color: COLORS.inputBg,
                            borderColor: COLORS.inputBorder,
                            borderWidth: 1,
                            children: [
                                InputEdgy({
                                    controller: titleCtrl,
                                    placeholder: "What needs doing?",
                                    fontSize: 14,
                                    color: COLORS.text,
                                    placeholderColor: COLORS.textMuted,
                                    cursorColor: COLORS.accent,
                                    queryKey: ["add-title"],
                                }),
                            ],
                        }),
                        SizedBox({ height: 14 }),
                        Text({
                            text: "Description",
                            fontSize: 11,
                            color: COLORS.textMuted,
                        }),
                        SizedBox({ height: 6 }),
                        Container({
                            borderRadius: 7,
                            padding: 9,
                            color: COLORS.inputBg,
                            borderColor: COLORS.inputBorder,
                            borderWidth: 1,
                            children: [
                                InputEdgy({
                                    controller: descCtrl,
                                    placeholder: "Optional details",
                                    fontSize: 14,
                                    color: COLORS.text,
                                    placeholderColor: COLORS.textMuted,
                                    cursorColor: COLORS.accent,
                                    queryKey: ["add-desc"],
                                }),
                            ],
                        }),
                        SizedBox({ height: 20 }),
                        Row({
                            mainAlignment: MainAxisAlignment.End,
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                Button({
                                    label: "Cancel",
                                    bg: COLORS.subtleButton,
                                    fg: COLORS.subtleButtonFg,
                                    onClick: closeAddModal,
                                }),
                                SizedBox({ width: 8 }),
                                Button({
                                    label: "Add Task",
                                    bg: COLORS.accent,
                                    fg: COLORS.accentFg,
                                    onClick: submitAdd,
                                }),
                            ],
                        }),
                    ],
                }),
            ],
        }),
    });
}

// --- ConfirmRemoveModal ---------------------------------------------------

export function ConfirmRemoveModal(): Element {
    return ModalShell({
        onBackdropClick: closeRemoveModal,
        card: Container({
            width: 340,
            borderRadius: 14,
            padding: 22,
            color: COLORS.cardBg,
            borderColor: COLORS.cardBorder,
            borderWidth: 1,
            shadowColor: COLORS.cardShadowLg,
            shadowBlur: 30,
            shadowOffset: [0, 12],
            children: [
                Column({
                    crossAlignment: CrossAxisAlignment.Stretch,
                    mainAxisSize: MainAxisSize.Min,
                    children: [
                        Text({
                            text: "Remove task?",
                            fontSize: 16,
                            color: COLORS.text,
                        }),
                        SizedBox({ height: 8 }),
                        Text({
                            text: derive(() => {
                                const idx = get(removeTarget$);
                                if (idx === null) return "";
                                const tasks = get(tasks$);
                                return tasks[idx]
                                    ? `"${tasks[idx].title}" will be permanently removed.`
                                    : "";
                            }),
                            fontSize: 13,
                            color: COLORS.textMuted,
                        }),
                        SizedBox({ height: 18 }),
                        Row({
                            mainAlignment: MainAxisAlignment.End,
                            mainAxisSize: MainAxisSize.Min,
                            children: [
                                Button({
                                    label: "Cancel",
                                    bg: COLORS.subtleButton,
                                    fg: COLORS.subtleButtonFg,
                                    onClick: closeRemoveModal,
                                }),
                                SizedBox({ width: 8 }),
                                Button({
                                    label: "Remove",
                                    bg: COLORS.danger,
                                    fg: COLORS.dangerFg,
                                    onClick: confirmRemove,
                                }),
                            ],
                        }),
                    ],
                }),
            ],
        }),
    });
}
