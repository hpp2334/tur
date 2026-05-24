import { atom, getDefaultStore } from "jotai/vanilla";
import { useAtomValue, useSetAtom } from "jotai/react";
import { createTextEditingController } from "@tur/react-renderer";

export interface Todo {
  readonly id: number;
  readonly text: string;
  readonly done: boolean;
  readonly description: string;
}

const INITIAL_TODOS: readonly Todo[] = [
  { id: 1, text: "Learn Rust", done: true, description: "Complete the Rust book and build a CLI tool" },
  { id: 2, text: "Build tur engine", done: false, description: "Implement the rendering engine with winit and vello" },
  { id: 3, text: "Write documentation", done: false, description: "Document the architecture and API surface" },
  { id: 4, text: "Ship v0.1.0", done: false, description: "First public release with core features" },
];

export const store = getDefaultStore();

export const todosAtom = atom<readonly Todo[]>(INITIAL_TODOS);

export const addTodoAtom = atom(null, (_get, set, payload: { text: string; description: string }) => {
  const trimmed = payload.text.trim();
  if (!trimmed) return;
  set(todosAtom, (prev) => [
    ...prev,
    { id: Date.now(), text: trimmed, description: payload.description, done: false },
  ]);
});

export const toggleTodoAtom = atom(null, (_get, set, id: number) => {
  set(todosAtom, (prev) =>
    prev.map((t) => (t.id === id ? { ...t, done: !t.done } : t)),
  );
});

export const removeTodoAtom = atom(null, (_get, set, id: number) => {
  set(todosAtom, (prev) => prev.filter((t) => t.id !== id));
});

export const selectedTodoIdAtom = atom<number | null>(null);

export const showModalAtom = atom(false);

export const titleTextAtom = atom("");
export const descTextAtom = atom("");

export const titleControllerAtom = atom(
  createTextEditingController({
    onInput: (text: string) => store.set(titleTextAtom, text),
  }),
);
export const descControllerAtom = atom(
  createTextEditingController({
    onInput: (text: string) => store.set(descTextAtom, text),
  }),
);

export { useAtomValue, useSetAtom };
