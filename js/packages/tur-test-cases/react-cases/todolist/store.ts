import { atom, createStore } from "jotai/vanilla";
import { useAtomValue, useSetAtom } from "jotai/react";

export interface Todo {
  readonly id: number;
  readonly text: string;
  readonly done: boolean;
}

const INITIAL_TODOS: readonly Todo[] = [
  { id: 1, text: "Learn Rust", done: true },
  { id: 2, text: "Build tur engine", done: false },
  { id: 3, text: "Write documentation", done: false },
  { id: 4, text: "Ship v0.1.0", done: false },
];

export const store = createStore();

export const todosAtom = atom<readonly Todo[]>(INITIAL_TODOS);

export const toggleTodoAtom = atom(null, (_get, set, id: number) => {
  set(todosAtom, (prev) =>
    prev.map((t) => (t.id === id ? { ...t, done: !t.done } : t)),
  );
});

export const removeTodoAtom = atom(null, (_get, set, id: number) => {
  set(todosAtom, (prev) => prev.filter((t) => t.id !== id));
});

export { useAtomValue, useSetAtom };
