import {
    Alignment,
    Color,
    Column,
    Condition,
    Container,
    CrossAxisAlignment,
    Expanded,
    MainAxisAlignment,
    Mutation,
    PointerInteract,
    PointerInteractEvent,
    ReadableSubscribe,
    Readable,
    Row,
    SizedBox,
    Text,
    derive,
    lifecycleView,
    mutate,
    source,
    view,
} from "@tur/edgy";

const count$ = source(0);
const mountedCount$ = source(0);
const destroyedCount$ = source(0);
const updateCount$ = source(0);
const visible$ = source(true);

export default view(() =>
    Expanded({
        child: Container({
            color: Color.hex("#f8fafc"),
            children: [
                Column({
                    mainAlignment: MainAxisAlignment.Center,
                    crossAlignment: CrossAxisAlignment.Center,
                    children: [
                        Text({
                            text: derive((g) => `Count: ${g(count$)}`),
                            fontSize: 32,
                            color: Color.hex("#0f172a"),
                        }),
                        Text({
                            text: derive((g) => `onUpdated: ${g(updateCount$)}`),
                            queryKey: ["update-count"],
                            fontSize: 18,
                            color: Color.hex("#1d4ed8"),
                        }),
                        Text({
                            text: derive((g) => `onMounted: ${g(mountedCount$)}`),
                            queryKey: ["mounted-count"],
                            fontSize: 18,
                            color: Color.hex("#16a34a"),
                        }),
                        Text({
                            text: derive((g) => `beforeDestroy: ${g(destroyedCount$)}`),
                            queryKey: ["destroyed-count"],
                            fontSize: 18,
                            color: Color.hex("#dc2626"),
                        }),
                        SizedBox({ height: 16 }),
                        Row({
                            mainAlignment: MainAxisAlignment.Center,
                            children: [
                                pill({
                                    label: "+1",
                                    queryKey: ["inc"],
                                    onClick: mutate(({ get, set }, _e: PointerInteractEvent) =>
                                        set(count$, get(count$) + 1),
                                    ),
                                }),
                                SizedBox({ width: 12 }),
                                pill({
                                    label: derive((g) => (g(visible$) ? "hide" : "show")),
                                    queryKey: ["toggle"],
                                    onClick: mutate(({ get, set }, _e: PointerInteractEvent) =>
                                        set(visible$, !get(visible$)),
                                    ),
                                }),
                            ],
                        }),
                        SizedBox({ height: 16 }),
                        Condition({
                            condition: derive((g) => g(visible$)),
                            child: () =>
                                lifecycleView(() => ({
                                    element: ReadableSubscribe({
                                        readables: [count$],
                                        onUpdate$: mutate(({ get, set }) =>
                                            set(updateCount$, get(updateCount$) + 1),
                                        ),
                                        child: Container({
                                            width: 220,
                                            height: 64,
                                            borderRadius: 8,
                                            color: Color.hex("#dbeafe"),
                                            alignment: Alignment.Center,
                                            children: [
                                                Text({
                                                    text: "subscribed",
                                                    fontSize: 18,
                                                    color: Color.hex("#1e3a8a"),
                                                    queryKey: ["subscribed"],
                                                }),
                                            ],
                                        }),
                                    }),
                                    onMounted$: mutate(({ get, set }) =>
                                        set(mountedCount$, get(mountedCount$) + 1),
                                    ),
                                    beforeDestroy$: mutate(({ get, set }) =>
                                        set(destroyedCount$, get(destroyedCount$) + 1),
                                    ),
                                })),
                            elseChild: () => SizedBox({ width: 220, height: 64 }),
                        }),
                    ],
                }),
            ],
        }),
    }),
);

function pill(opts: {
    label: string | Readable<string>;
    queryKey?: string[];
    onClick: Mutation<[PointerInteractEvent]>;
}) {
    return PointerInteract({
        queryKey: opts.queryKey,
        onClick: opts.onClick,
        child: Container({
            width: 96,
            height: 44,
            borderRadius: 8,
            color: Color.hex("#6366f1"),
            alignment: Alignment.Center,
            children: [
                Text({
                    text: opts.label,
                    fontSize: 18,
                    color: Color.hex("#ffffff"),
                }),
            ],
        }),
    });
}
