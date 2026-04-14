import { For } from "solid-js";
import { Column, Row, Text, Container, SizedBox, CrossAxisAlignment, MainAxisAlignment } from "@tur/solidjs";
import { createTodoStore } from "./store";

export function TodoList() {
  const { todos } = createTodoStore();

  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="Todo List" fontSize={24} />
        <SizedBox height={16} />
        <Column>
          <For each={todos()}>
            {(todo) => (
              <Row mainAlignment={MainAxisAlignment.SpaceBetween}>
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
