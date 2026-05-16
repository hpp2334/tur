import { useState } from "react";

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
] as const;

export function createTodoStore() {
  const [todos, setTodos] = useState<readonly Todo[]>(INITIAL_TODOS);

  const addTodo = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed) return;
    setTodos((prev) => [
      ...prev,
      { id: Date.now(), text: trimmed, done: false },
    ]);
  };

  const toggleTodo = (id: number) => {
    setTodos((prev) =>
      prev.map((t) => (t.id === id ? { ...t, done: !t.done } : t)),
    );
  };

  const removeTodo = (id: number) => {
    setTodos((prev) => prev.filter((t) => t.id !== id));
  };

  return { todos, addTodo, toggleTodo, removeTodo };
}
