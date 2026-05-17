import { useState } from "react";
import {
  Column,
  Row,
  Text,
  Container,
  SizedBox,
  PointerInteract,
  MainAxisAlignment,
  CrossAxisAlignment,
  Input,
  InputController,
} from "@tur/react";
import { todosAtom, toggleTodoAtom, removeTodoAtom, useAtomValue, useSetAtom } from "./store";
import { Sidebar } from "../../components/Sidebar";

function TodoItem(props: { todo: { id: number; text: string; done: boolean }; toggle: (id: number) => void; remove: (id: number) => void }) {
  return (
    <Row mainAlignment={MainAxisAlignment.SpaceBetween} crossAlignment={CrossAxisAlignment.Start}>
      <PointerInteract onClick={() => props.toggle(props.todo.id)} child={
        <Text content={props.todo.done ? "[x]" : "[ ]"} />
      } />
      <Text content={props.todo.text} />
      <PointerInteract onClick={() => props.remove(props.todo.id)} child={
        <Container padding={4} color={"#e74c3c" as unknown as import("@tur/react-renderer").Color}>
          <Text content={"[x]"} />
        </Container>
      } />
    </Row>
  );
}

function TodoList() {
  const todos = useAtomValue(todosAtom);
  const toggleTodo = useSetAtom(toggleTodoAtom);
  const removeTodo = useSetAtom(removeTodoAtom);
  const [inputText, setInputText] = useState("");

  const controller = new InputController({
    onInput: (text: string) => {
      setInputText(text);
    },
    onKeyDown: (e) => {
      if (e.key === "Enter") {
        const text = inputText.trim();
        if (text) {
          controller.clear();
          setInputText("");
        }
      }
    },
  });

  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="Todo List" fontSize={24} />
        <SizedBox height={16} />
        <Input controller={controller} placeholder="Add a task..." fontSize={14} width={200} height={30} />
        <SizedBox height={16} />
        <Column>
          {todos.map((todo) => (
            <TodoItem todo={todo} toggle={toggleTodo} remove={removeTodo} />
          ))}
        </Column>
      </Column>
    </Container>
  );
}

const TABS = [{ id: "todolist", label: "TodoList" }];

export function App() {
  return (
    <Row>
      <Sidebar tabs={TABS} activeId="todolist" />
      <TodoList />
    </Row>
  );
}
