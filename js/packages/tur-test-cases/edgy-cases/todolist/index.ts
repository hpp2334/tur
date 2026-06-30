import {
    Color,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    view,
    derive,
    Each,
    type EdgyElement,
    Expanded,
    get,
    ImageEdgy,
    MainAxisAlignment,
    MainAxisSize,
    MouseRegion,
    mutate,
    PointerInteract,
    Positioned,
    Row,
    ScrollView,
    SizedBox,
    Stack,
    Text,
} from "@tur/edgy";
import { AddTaskModal, ConfirmRemoveModal, TaskItem } from "./views";
import {
    addOpen$,
    getIcon,
    openAddModal,
    removeTarget$,
    tasks$,
} from "./state";

// --- Light theme palette (Notion / Linear-style) -------------------------

const COLORS = {
    pageBg: Color.hex("#f8fafc"), // slate-50
    text: Color.hex("#0f172a"), // slate-900
    textMuted: Color.hex("#64748b"), // slate-500
    accent: Color.hex("#4f46e5"), // indigo-600
    accentFg: Color.hex("#ffffff"),
};

function Header(): EdgyElement {
    return Row({
        mainAlignment: MainAxisAlignment.SpaceBetween,
        crossAlignment: CrossAxisAlignment.Center,
        // Row's main axis is horizontal and we *do* want it to fill the width
        // so SpaceBetween can push the button to the right edge. We do NOT
        // want its children to fill vertically — the inner Column below uses
        // `MainAxisSize.Min` so it hugs its text content; otherwise it would
        // expand to the parent's max height (the engine default) and starve
        // the TaskList `Expanded` slot to zero.
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Start,
                mainAxisSize: MainAxisSize.Min,
                children: [
                    Text({
                        text: "Tasks",
                        fontSize: 26,
                        color: COLORS.text,
                    }),
                    SizedBox({ height: 4 }),
                    Text({
                        text: derive(() => {
                            const tasks = get(tasks$);
                            const done = tasks.filter(
                                (t) => t.completed,
                            ).length;
                            return `${tasks.length} items · ${done} done`;
                        }),
                        fontSize: 13,
                        color: COLORS.textMuted,
                    }),
                ],
            }),
            MouseRegion({
                cursor: "pointer",
                child: PointerInteract({
                    onClick: mutate((ctx, _ev) => openAddModal(ctx)),
                    child: Container({
                        padding: 10,
                        borderRadius: 8,
                        color: COLORS.accent,
                        shadowColor: Color.rgba(79, 70, 229, 80),
                        shadowBlur: 10,
                        shadowOffset: [0, 4],
                        children: [
                            Row({
                                mainAxisSize: MainAxisSize.Min,
                                crossAlignment: CrossAxisAlignment.Center,
                                children: [
                                    ImageEdgy({
                                        resourceId: getIcon("plus"),
                                        width: 13,
                                        height: 13,
                                        queryKey: ["plus-icon"],
                                    }),
                                    SizedBox({ width: 7 }),
                                    Text({
                                        text: "New Task",
                                        fontSize: 13,
                                        color: COLORS.accentFg,
                                    }),
                                ],
                            }),
                        ],
                    }),
                }),
            }),
        ],
    });
}

function TaskList(): EdgyElement {
    return Expanded({
        child: ScrollView({
            child: Column({
                crossAlignment: CrossAxisAlignment.Stretch,
                children: [
                    Each({
                        items: tasks$,
                        build: (task, index) =>
                            Column({
                                crossAlignment: CrossAxisAlignment.Stretch,
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    index === 0
                                        ? SizedBox({ width: 0, height: 0 })
                                        : SizedBox({ height: 10 }),
                                    TaskItem({ task, index }),
                                ],
                            }),
                    }),
                ],
            }),
        }),
    });
}

export default view(() =>
    Expanded({
        child: Stack({
            children: [
                Container({
                    color: COLORS.pageBg,
                    padding: 22,
                    children: [
                        Column({
                            crossAlignment: CrossAxisAlignment.Stretch,
                            children: [
                                Header(),
                                SizedBox({ height: 18 }),
                                TaskList(),
                            ],
                        }),
                    ],
                }),
                Condition({
                    condition: addOpen$,
                    child: () =>
                        Positioned({
                            top: 0,
                            left: 0,
                            right: 0,
                            bottom: 0,
                            child: AddTaskModal(),
                        }),
                }),
                Condition({
                    condition: derive(() => get(removeTarget$) !== null),
                    child: () =>
                        Positioned({
                            top: 0,
                            left: 0,
                            right: 0,
                            bottom: 0,
                            child: ConfirmRemoveModal(),
                        }),
                }),
            ],
        }),
    }),
);
