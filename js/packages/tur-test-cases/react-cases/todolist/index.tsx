import { renderRoot } from "@tur/react-renderer";
import type { TurKeyEvent } from '@tur/react';
import {
  Column,
  Row,
  Text,
  Container,
  SizedBox,
  CrossAxisAlignment,
  MainAxisAlignment,
  Input,
  PointerInteract,
  createTextEditingController,
} from "@tur/react";
import {
  todosAtom,
  toggleTodoAtom,
  removeTodoAtom,
  selectedTodoIdAtom,
  useAtomValue,
  useSetAtom,
} from "./store";

function TodoItem(props: { todo: { id: number; text: string; done: boolean }; onToggle: (id: number) => void; onRemove: (id: number) => void; onSelect: (id: number) => void; isSelected: boolean }) {
  return (
    <PointerInteract onClick={() => props.onSelect(props.todo.id)} child={
      <Container queryKey={["todo-item", String(props.todo.id), props.isSelected ? "selected" : "unselected"]} padding={4}>
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
      </Container>
    } />
  );
}

function TodoList() {
  const todos = useAtomValue(todosAtom);
  const toggleTodo = useSetAtom(toggleTodoAtom);
  const removeTodo = useSetAtom(removeTodoAtom);
  const selectedTodoId = useAtomValue(selectedTodoIdAtom);
  const setSelectedTodoId = useSetAtom(selectedTodoIdAtom);

  const controller = createTextEditingController({
    onKeyDown: (e: TurKeyEvent) => {
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
        <Text content={`Count: ${todos.length}`} queryKey={["count"]} />
        <Column queryKey={["todo-list"]}>
          {todos.map((todo) => (
            <TodoItem todo={todo} onToggle={toggleTodo} onRemove={removeTodo} onSelect={setSelectedTodoId} isSelected={todo.id === selectedTodoId} />
          ))}
        </Column>
      </Column>
    </Container>
  );
}

renderRoot(TodoList);
