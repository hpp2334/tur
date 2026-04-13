import { For } from "solid-js";
import {
  Column,
  Row,
  Text,
  Container,
  SizedBox,
} from "@tur/solidjs-renderer/jsx-runtime";
import { createTodoStore } from "./store";

export function TodoList() {
  const { todos } = createTodoStore();

  return (
    <Container padding={16}>
      <Column crossAlignment="center">
        <Text content="Todo List" fontSize={24} />
        <SizedBox height={16} />
        <Column>
          <For each={todos()}>
            {(todo) => (
              <Row mainAlignment="space-between">
                <Text content={todo.text} />
                <Text content={todo.done ? "\u2713" : "\u25CB"} />
              </Row>
            )}
          </For>
        </Column>
      </Column>
    </Container>
  );
}
