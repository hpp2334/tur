import { createSignal } from "solid-js";
import { renderRoot } from "@tur/solidjs-renderer";
import {
  Column,
  Row,
  Text,
  Container,
  SizedBox,
  CrossAxisAlignment,
  MainAxisAlignment,
  Input,
  InputController,
  PointerInteract,
} from "@tur/solidjs";

interface Todo {
  id: number;
  text: string;
  done: boolean;
}

const INITIAL_TODOS: Todo[] = [
  { id: 1, text: "Learn Rust", done: true },
  { id: 2, text: "Build tur engine", done: false },
  { id: 3, text: "Write documentation", done: false },
  { id: 4, text: "Ship v0.1.0", done: false },
];

function TodoItem(props: { todo: Todo; onToggle: (id: number) => void; onRemove: (id: number) => void }) {
  return (
    <Row mainAlignment={MainAxisAlignment.SpaceBetween} crossAlignment={CrossAxisAlignment.Start}>
      <PointerInteract
        onClick={() => props.onToggle(props.todo.id)}
        child={
          <Text
            content={props.todo.done ? "\u2713" : "\u25CB"}
            queryKey={["toggle", String(props.todo.id)]}
          />
        }
      />
      <Text content={props.todo.text} />
      <PointerInteract
        onClick={() => props.onRemove(props.todo.id)}
        child={
          <Container queryKey={["remove", String(props.todo.id)]} padding={4}>
            <Text content={"\u2715"} />
          </Container>
        }
      />
    </Row>
  );
}

function TodoList() {
  const [todos, setTodos] = createSignal<Todo[]>(INITIAL_TODOS);

  const toggleTodo = (id: number) => {
    setTodos((prev) => prev.map((t) => (t.id === id ? { ...t, done: !t.done } : t)));
  };

  const removeTodo = (id: number) => {
    setTodos((prev) => prev.filter((t) => t.id !== id));
  };

  const controller = new InputController({
    onKeyDown: (e) => {
      if (e.key === "Enter") {
        controller.clear();
      }
    },
  });

  return (
    <Container padding={16} queryKey={["root"]}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="Todo List" fontSize={24} />
        <SizedBox height={16} />
        <Container queryKey={["input-wrapper"]}>
          <Input controller={controller} placeholder="Add a task..." fontSize={14} width={200} height={30} />
        </Container>
        <SizedBox height={16} />
        <Column queryKey={["todo-list"]}>
          {todos().map((todo) => (
            <TodoItem todo={todo} onToggle={toggleTodo} onRemove={removeTodo} />
          ))}
        </Column>
      </Column>
    </Container>
  );
}

renderRoot(TodoList);
