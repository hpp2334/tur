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

declare global {
  var __toggleRefs: Record<number, any>;
}

globalThis.__toggleRefs = {};

function TodoItem(props: { todo: Todo }) {
  const todo = props.todo;
  return (
    <Row mainAlignment={MainAxisAlignment.SpaceBetween} crossAlignment={CrossAxisAlignment.Start}>
      <tur_pointer_interact
        ref={(el: any) => { globalThis.__toggleRefs[todo.id] = el; }}
        onClick={() => {
          todo.done = !todo.done;
          const ctx = __tur.__ctx;
          const piHandle = globalThis.__toggleRefs[todo.id];
          if (piHandle) {
            const textContainer = __tur.getFirstChild(ctx, piHandle);
            if (textContainer) {
              const span = __tur.getFirstChild(ctx, textContainer);
              if (span) {
                __tur.setAttribute(ctx, span, "content", todo.done ? "\u2713" : "\u25CB");
              }
            }
          }
        }}
      >
        <Text
          content={todo.done ? "\u2713" : "\u25CB"}
          queryKey={["toggle", String(todo.id)]}
        />
      </tur_pointer_interact>
      <Text content={todo.text} />
      <tur_pointer_interact onClick={() => {}}>
        <Container queryKey={["remove", String(todo.id)]} padding={4}>
          <Text content={"✕"} />
        </Container>
      </tur_pointer_interact>
    </Row>
  );
}

function TodoList() {
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
          {INITIAL_TODOS.map((todo) => (
            <TodoItem todo={todo} />
          ))}
        </Column>
      </Column>
    </Container>
  );
}

renderRoot(TodoList);
