import {
    Color,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    component,
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
import { AddTaskModal, ConfirmRemoveModal, TaskItem } from "./components";
import {
    addOpen$,
    getIcon,
    openAddModal,
    removeTarget$,
    tasks$,
} from "./state";

const COLORS = {
    pageBg: Color.hex("#0f172a"),
    text: Color.hex("#f8fafc"),
    textMuted: Color.hex("#94a3b8"),
    accent: Color.hex("#4f46e5"),
};

function Header(): EdgyElement {
    return Row({
        mainAlignment: MainAxisAlignment.SpaceBetween,
        crossAlignment: CrossAxisAlignment.Center,
        children: [
            Column({
                crossAlignment: CrossAxisAlignment.Start,
                children: [
                    Text({
                        text: "Tasks",
                        fontSize: 28,
                        color: COLORS.text,
                    }),
                    SizedBox({ height: 4 }),
                    Text({
                        text: derive(() => {
                            const tasks = get(tasks$);
                            const done = tasks.filter(
                                (t) => t.completed,
                            ).length;
                            return `${tasks.length} items · ${done} completed`;
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
                        padding: 11,
                        borderRadius: 8,
                        color: COLORS.accent,
                        children: [
                            Row({
                                mainAxisSize: MainAxisSize.Min,
                                children: [
                                    ImageEdgy({
                                        resourceId: getIcon("plus"),
                                        width: 14,
                                        height: 14,
                                        queryKey: ["plus-icon"],
                                    }),
                                    SizedBox({ width: 8 }),
                                    Text({
                                        text: "New Task",
                                        fontSize: 13,
                                        color: Color.hex("#ffffff"),
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
                                children: [
                                    index === 0
                                        ? SizedBox({ width: 0, height: 0 })
                                        : SizedBox({ height: 8 }),
                                    TaskItem({ task, index }),
                                ],
                            }),
                    }),
                ],
            }),
        }),
    });
}

export default component(() =>
    Expanded({
        child: Stack({
            children: [
                Container({
                    color: COLORS.pageBg,
                    padding: 24,
                    children: [
                        Column({
                            crossAlignment: CrossAxisAlignment.Stretch,
                            children: [
                                Header(),
                                SizedBox({ height: 20 }),
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
