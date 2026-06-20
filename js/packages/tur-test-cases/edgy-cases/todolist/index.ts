import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    component,
    Each,
    Expanded,
    get,
    MainAxisAlignment,
    MainAxisSize,
    mutate,
    PointerInteract,
    Row,
    SizedBox,
    set,
    source,
    Text,
} from "@tur/edgy";

interface Task {
    title: string;
    completed: boolean;
}

const tasks$ = source<Task[]>([
    { title: "Buy groceries", completed: false },
    { title: "Walk the dog", completed: true },
    { title: "Finish report", completed: false },
    { title: "Call mom", completed: false },
]);

function TaskItem({ task, index }: { task: Task; index: number }) {
    return Row({
        mainAlignment: MainAxisAlignment.SpaceBetween,
        children: [
            Row({
                mainAxisSize: MainAxisSize.Min,
                children: [
                    PointerInteract({
                        onClick: mutate(({ get, set }, _ev) => {
                            const tasks = get(tasks$);
                            set(
                                tasks$,
                                tasks.map((t, i) =>
                                    i === index
                                        ? { ...t, completed: !t.completed }
                                        : t,
                                ),
                            );
                        }),
                        child: Container({
                            width: 24,
                            height: 24,
                            alignment: Alignment.Center,
                            color: task.completed
                                ? Color.hex("#22c55e")
                                : Color.hex("#334155"),
                            children: [
                                Text({
                                    text: task.completed ? "x" : "",
                                    fontSize: 12,
                                    color: Color.hex("#ffffff"),
                                }),
                            ],
                        }),
                    }),
                    SizedBox({ width: 12 }),
                    Text({
                        text: task.title,
                        fontSize: 16,
                        color: task.completed
                            ? Color.hex("#64748b")
                            : Color.hex("#e2e8f0"),
                    }),
                ],
            }),
            PointerInteract({
                onClick: mutate(({ get, set }, _ev) => {
                    const tasks = get(tasks$);
                    set(
                        tasks$,
                        tasks.filter((_, i) => i !== index),
                    );
                }),
                child: Container({
                    width: 32,
                    height: 32,
                    alignment: Alignment.Center,
                    color: Color.hex("#dc2626"),
                    children: [
                        Text({
                            text: "x",
                            fontSize: 14,
                            color: Color.hex("#ffffff"),
                        }),
                    ],
                }),
            }),
        ],
    });
}

export default component(() =>
    Expanded({
        child: Container({
            color: Color.hex("#0f172a"),
            children: [
                Row({
                    children: [
                        Container({
                            color: Color.hex("#1e293b"),
                            width: 200,
                            children: [
                                Column({
                                    children: [
                                        Container({
                                            padding: 16,
                                            children: [
                                                Text({
                                                    text: "Tur Todo",
                                                    fontSize: 20,
                                                    color: Color.hex("#f8fafc"),
                                                }),
                                            ],
                                        }),
                                        Container({
                                            padding: 12,
                                            children: [
                                                Text({
                                                    text: "My Tasks",
                                                    fontSize: 14,
                                                    color: Color.hex("#94a3b8"),
                                                }),
                                            ],
                                        }),
                                    ],
                                }),
                            ],
                        }),
                        Expanded({
                            child: Container({
                                padding: 16,
                                children: [
                                    Column({
                                        children: [
                                            Row({
                                                mainAlignment:
                                                    MainAxisAlignment.SpaceBetween,
                                                children: [
                                                    Text({
                                                        text: "TodoList",
                                                        fontSize: 24,
                                                        color: Color.hex(
                                                            "#f8fafc",
                                                        ),
                                                    }),
                                                    PointerInteract({
                                                        onClick: mutate(
                                                            (
                                                                { get, set },
                                                                _ev,
                                                            ) => {
                                                                const tasks =
                                                                    get(tasks$);
                                                                set(tasks$, [
                                                                    ...tasks,
                                                                    {
                                                                        title: "New task",
                                                                        completed: false,
                                                                    },
                                                                ]);
                                                            },
                                                        ),
                                                        child: Container({
                                                            color: Color.hex(
                                                                "#4f46e5",
                                                            ),
                                                            padding: 8,
                                                            children: [
                                                                Text({
                                                                    text: "+ New Task",
                                                                    fontSize: 14,
                                                                    color: Color.hex(
                                                                        "#ffffff",
                                                                    ),
                                                                }),
                                                            ],
                                                        }),
                                                    }),
                                                ],
                                            }),
                                            SizedBox({ height: 16 }),
                                            Each<Task>({
                                                items: tasks$,
                                                build: (task, index) =>
                                                    TaskItem({ task, index }),
                                            }),
                                        ],
                                    }),
                                ],
                            }),
                        }),
                    ],
                }),
            ],
        }),
    }),
);
