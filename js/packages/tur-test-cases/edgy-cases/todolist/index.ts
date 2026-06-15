import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    Expanded,
    MainAxisAlignment,
    PointerInteract,
    Row,
    SizedBox,
    Text,
    render,
} from "@tur/edgy";

interface Task {
    title: string;
    completed: boolean;
}

const TASKS: Task[] = [
    { title: "Buy groceries", completed: false },
    { title: "Walk the dog", completed: true },
    { title: "Finish report", completed: false },
    { title: "Call mom", completed: false },
];

function TaskItem({ task }: { task: Task }) {
    return Row({
        mainAlignment: MainAxisAlignment.SpaceBetween,
        children: [
            Row({
                children: [
                    PointerInteract({
                        child: Container({
                            width: 24,
                            height: 24,
                            color: task.completed ? Color.hex("#22c55e") : Color.hex("#334155"),
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
                        color: task.completed ? Color.hex("#64748b") : Color.hex("#e2e8f0"),
                    }),
                ],
            }),
            PointerInteract({
                child: Container({
                    width: 32,
                    height: 32,
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

render(() =>
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
                                                mainAlignment: MainAxisAlignment.SpaceBetween,
                                                children: [
                                                    Text({
                                                        text: "TodoList",
                                                        fontSize: 24,
                                                        color: Color.hex("#f8fafc"),
                                                    }),
                                                    PointerInteract({
                                                        child: Container({
                                                            color: Color.hex("#4f46e5"),
                                                            padding: 8,
                                                            children: [
                                                                Text({
                                                                    text: "+ New Task",
                                                                    fontSize: 14,
                                                                    color: Color.hex("#ffffff"),
                                                                }),
                                                            ],
                                                        }),
                                                    }),
                                                ],
                                            }),
                                            SizedBox({ height: 16 }),
                                            ...TASKS.map((t) => TaskItem({ task: t })),
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
