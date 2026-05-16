import { InputController } from "@tur/react-renderer";
import { useState, useEffect, useCallback } from "react";

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

let _nextId = 0;

function primitiveAtom<T>(initialValue: T) {
  return { _id: ++_nextId, init: initialValue };
}

function writableAtom(
  write: (get: (id: number) => unknown, set: (id: number, v: unknown) => void, arg: unknown) => void,
) {
  return { _id: ++_nextId, init: null, _write: write };
}

function createStore() {
  const state: Record<number, unknown> = {};
  const subs: Record<number, Array<() => void>> = {};

  function doGet(id: number): unknown {
    if (!(id in state)) return undefined;
    return state[id];
  }

  function doGetAtom(a: { _id: number; init: unknown }): unknown {
    if (!(a._id in state)) {
      state[a._id] = a.init;
    }
    return state[a._id];
  }

  function doSetAtom(a: { _id: number; init: unknown; _write?: any }, value: unknown): void {
    if (a._write) {
      a._write(doGetAtom, doSetAtom, value);
      return;
    }
    const current = doGetAtom(a);
    const next = typeof value === "function" ? (value as (p: unknown) => unknown)(current) : value;
    state[a._id] = next;
    const listeners = subs[a._id];
    if (listeners) {
      for (let i = 0; i < listeners.length; i++) listeners[i]();
    }
  }

  function doSub(a: { _id: number }, listener: () => void): () => void {
    if (!subs[a._id]) subs[a._id] = [];
    subs[a._id].push(listener);
    return () => {
      const arr = subs[a._id];
      if (!arr) return;
      const idx = arr.indexOf(listener);
      if (idx >= 0) arr.splice(idx, 1);
    };
  }

  return { get: doGetAtom, set: doSetAtom, sub: doSub };
}

export const store = createStore();

globalThis.__debugGetTodos = function() {
  return JSON.stringify(store.get(todosAtom));
};

export const todosAtom = primitiveAtom<readonly Todo[]>(INITIAL_TODOS);

export const addTodoAtom = writableAtom(
  (_get, set, text: unknown) => {
    const trimmed = (text as string).trim();
    if (!trimmed) return;
    set(todosAtom, (prev: unknown) => [
      ...(prev as readonly Todo[]),
      { id: Date.now(), text: trimmed, done: false },
    ]);
  },
);

export const toggleTodoAtom = writableAtom(
  (_get, set, id: unknown) => {
    const numId = id as number;
    set(todosAtom, (prev: unknown) =>
      (prev as readonly Todo[]).map((t) => (t.id === numId ? { ...t, done: !t.done } : t)),
    );
  },
);

export const removeTodoAtom = writableAtom(
  (_get, set, id: unknown) => {
    const numId = id as number;
    set(todosAtom, (prev: unknown) =>
      (prev as readonly Todo[]).filter((t) => t.id !== numId),
    );
  },
);

const inputTextAtom = primitiveAtom("");

export const inputControllerAtom = primitiveAtom(
  new InputController({
    onInput: (text: string) => {
      store.set(inputTextAtom, text);
    },
    onKeyDown: (e: { key: string }) => {
      if (e.key === "Enter") {
        const text = store.get(inputTextAtom) as string;
        store.set(addTodoAtom, text);
        store.set(inputTextAtom, "");
        controller.clear();
      }
    },
  }),
);

const controller = store.get(inputControllerAtom) as InputController;

export function useAtomValue<T>(a: { _id: number; init: unknown }): T {
  const [value, setValue] = useState(() => store.get(a) as T);
  useEffect(() => {
    globalThis.__subCount = (globalThis.__subCount || 0) + 1;
    return store.sub(a, () => {
      globalThis.__notifyCount = (globalThis.__notifyCount || 0) + 1;
      setValue(store.get(a) as T);
    });
  }, [a._id]);
  return value;
}

export function useSetAtom(
  a: { _id: number; init: unknown; _write?: any },
): (arg: unknown) => void {
  return useCallback((arg: unknown) => store.set(a, arg), [a._id]);
}
