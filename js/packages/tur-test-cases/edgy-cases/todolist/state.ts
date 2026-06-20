import {
    type Atom,
    createSvgResource,
    createTextEditingController,
    mutate,
    type StoreCtx,
    source,
} from "@tur/edgy";

export interface Task {
    title: string;
    description: string;
    completed: boolean;
}

export const tasks$ = source<Task[]>([
    {
        title: "Buy groceries",
        description: "Milk, eggs, sourdough, coffee beans",
        completed: false,
    },
    {
        title: "Walk the dog",
        description: "30 minutes around the park",
        completed: true,
    },
    {
        title: "Finish quarterly report",
        description: "Q2 numbers + projection slides for Monday's review",
        completed: false,
    },
    {
        title: "Call mom",
        description: "Birthday planning for Saturday",
        completed: false,
    },
    {
        title: "Read chapter 4",
        description: "Designing Data-Intensive Applications",
        completed: false,
    },
]);

// --- Modal + draft state ---------------------------------------------------

export const addOpen$ = source(false);
export const removeTarget$ = source<number | null>(null);

export const titleDraft$ = source("");
export const descDraft$ = source("");

// --- Controllers -----------------------------------------------------------
//
// Created at module load. Reset (via setSpans) whenever the modal opens so
// the fields start blank.

interface TextController {
    setSpans(spans: Array<{ content: string; color?: unknown }>): void;
    readonly text: string;
}

const noopSpans = (s: string): Array<{ content: string }> => [{ content: s }];

export const titleCtrl = createTextEditingController({
    onInput: mutate((ctx: StoreCtx, text: string, enter: boolean) => {
        ctx.set(titleDraft$, text);
        if (enter) submitAdd(ctx);
    }),
    onKeyDown: mutate((ctx: StoreCtx, ev) => {
        if (ev.key === "Escape") closeAddModal(ctx);
    }),
}) as unknown as TextController;

export const descCtrl = createTextEditingController({
    onInput: mutate((ctx: StoreCtx, text: string, enter: boolean) => {
        ctx.set(descDraft$, text);
        if (enter) submitAdd(ctx);
    }),
    onKeyDown: mutate((ctx: StoreCtx, ev) => {
        if (ev.key === "Escape") closeAddModal(ctx);
    }),
}) as unknown as TextController;

// --- Modal actions ---------------------------------------------------------
//
// Plain functions taking the StoreCtx — callable from any mutation wrapper
// regardless of the event type (click, key, etc).

export function openAddModal(ctx: StoreCtx): void {
    titleCtrl.setSpans(noopSpans(""));
    descCtrl.setSpans(noopSpans(""));
    ctx.set(titleDraft$, "");
    ctx.set(descDraft$, "");
    ctx.set(addOpen$, true);
}

export function closeAddModal(ctx: StoreCtx): void {
    ctx.set(addOpen$, false);
}

export function submitAdd(ctx: StoreCtx): void {
    const title = ctx.get(titleDraft$).trim();
    if (!title) return;
    ctx.set(tasks$, [
        ...ctx.get(tasks$),
        {
            title,
            description: ctx.get(descDraft$).trim(),
            completed: false,
        },
    ]);
    ctx.set(addOpen$, false);
}

export function closeRemoveModal(ctx: StoreCtx): void {
    ctx.set(removeTarget$, null);
}

export function confirmRemove(ctx: StoreCtx): void {
    const idx = ctx.get(removeTarget$);
    if (idx === null) return;
    ctx.set(
        tasks$,
        ctx.get(tasks$).filter((_, i) => i !== idx),
    );
    ctx.set(removeTarget$, null);
}

export function toggleTask(ctx: StoreCtx, index: number): void {
    const tasks = ctx.get(tasks$);
    ctx.set(
        tasks$,
        tasks.map((t, i) =>
            i === index ? { ...t, completed: !t.completed } : t,
        ),
    );
}

export function requestRemove(ctx: StoreCtx, index: number): void {
    ctx.set(removeTarget$, index);
}

// Re-export for type-only imports in components.
export type { Atom };

// --- SVG icon resources ----------------------------------------------------
//
// Small inline SVGs rasterised up front via `createSvgResource` and stored as
// regular image resources. Looked up by name from components via `getIcon`.

const ICON_SVGS: Record<string, string> = {
    check: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>`,
    close: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#94a3b8" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`,
    plus: `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#ffffff" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>`,
};

const iconIds: Record<string, number> = {};
for (const name of Object.keys(ICON_SVGS)) {
    iconIds[name] = createSvgResource(ICON_SVGS[name]);
}

export function getIcon(name: "check" | "close" | "plus"): number {
    return iconIds[name];
}
