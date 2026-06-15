import {
    Column,
    Condition,
    Container,
    createTextEditingController,
    InputEdgy,
    MainAxisAlignment,
    PointerInteract,
    Row,
    Text,
    derive,
    mutate,
    render,
    source,
} from "@tur/edgy";

const DEFAULT_TIME = 60;

const remaining$ = source(DEFAULT_TIME);
const running$ = source(false);
const editing$ = source(false);
const initial$ = source(DEFAULT_TIME);
const editText$ = source("");
const editController$ = source<unknown>(null);

let timerId: ReturnType<typeof setInterval> | null = null;

const start$ = mutate(({ get, set }) => {
    if (get(running$)) return;
    set(running$, true);
    timerId = setInterval(() => {
        const r = get(remaining$);
        if (r <= 1) {
            if (timerId !== null) {
                clearInterval(timerId);
                timerId = null;
            }
            set(running$, false);
            set(remaining$, 0);
            return;
        }
        set(remaining$, r - 1);
    }, 1000);
});

const pause$ = mutate(({ get, set }) => {
    if (!get(running$)) return;
    if (timerId !== null) {
        clearInterval(timerId);
        timerId = null;
    }
    set(running$, false);
});

const reset$ = mutate(({ get, set }) => {
    if (timerId !== null) {
        clearInterval(timerId);
        timerId = null;
    }
    set(running$, false);
    set(remaining$, get(initial$));
});

const openEdit$ = mutate(({ get, set }) => {
    if (timerId !== null) {
        clearInterval(timerId);
        timerId = null;
    }
    set(running$, false);
    set(editText$, String(get(initial$)));
    const ctrl = createTextEditingController({
        onInput: (text: string) => set(editText$, text),
    });
    set(editController$, ctrl);
    set(editing$, true);
});

const confirmEdit$ = mutate(({ get, set }) => {
    const parsed = parseInt(get(editText$), 10);
    if (!Number.isNaN(parsed) && parsed > 0) {
        set(initial$, parsed);
        set(remaining$, parsed);
    }
    set(editing$, false);
    set(editController$, null);
});

render(() =>
    Container({
        padding: 16,
        queryKey: ["root"],
        children: [
            Column({
                children: [
                    Text({
                        text: derive((g) => `Countdown: ${g(remaining$)}`),
                        queryKey: ["display"],
                    }),
                    Row({
                        mainAlignment: MainAxisAlignment.Start,
                        children: [
                            PointerInteract({
                                onClick: openEdit$,
                                child: Container({
                                    padding: 8,
                                    queryKey: ["btn-edit"],
                                    children: [Text({ text: "Edit" })],
                                }),
                            }),
                            PointerInteract({
                                onClick: start$,
                                child: Container({
                                    padding: 8,
                                    queryKey: ["btn-start"],
                                    children: [Text({ text: "Start" })],
                                }),
                            }),
                            PointerInteract({
                                onClick: pause$,
                                child: Container({
                                    padding: 8,
                                    queryKey: ["btn-pause"],
                                    children: [Text({ text: "Pause" })],
                                }),
                            }),
                            PointerInteract({
                                onClick: reset$,
                                child: Container({
                                    padding: 8,
                                    queryKey: ["btn-reset"],
                                    children: [Text({ text: "Reset" })],
                                }),
                            }),
                        ],
                    }),
                    Condition({
                        condition: derive((g) => !!g(editing$)),
                        child: Container({
                            padding: 16,
                            queryKey: ["modal"],
                            children: [
                                Column({
                                    children: [
                                        Text({ text: "Set time:" }),
                                        Container({
                                            queryKey: ["edit-input"],
                                            children: [
                                                InputEdgy({
                                                    controller: derive((g) => g(editController$)),
                                                    placeholder: "Positive integer",
                                                    fontSize: 14,
                                                    width: 200,
                                                    height: 30,
                                                }),
                                            ],
                                        }),
                                        PointerInteract({
                                            onClick: confirmEdit$,
                                            child: Container({
                                                padding: 8,
                                                queryKey: ["btn-confirm"],
                                                children: [Text({ text: "Confirm" })],
                                            }),
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
);
