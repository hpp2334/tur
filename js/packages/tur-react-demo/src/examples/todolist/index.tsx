import { useState } from "react";
import {
  Column,
  Row,
  Text,
  Container,
  SizedBox,
  Stack,
  Positioned,
  PointerInteract,
  Expanded,
  MainAxisAlignment,
  MainAxisSize,
  CrossAxisAlignment,
  Input,
  InputController,
  BorderPosition,
} from "@tur/react";
import { Colors } from "../../theme";
import {
  type Todo,
  todosAtom,
  toggleTodoAtom,
  removeTodoAtom,
  addTodoAtom,
  selectedTodoIdAtom,
  useAtomValue,
  useSetAtom,
} from "./store";
import { Sidebar } from "../../components/Sidebar";

function Checkbox(props: { checked: boolean }) {
  if (props.checked) {
    return (
      <Container
        width={20}
        height={20}
        borderRadius={6}
        color={Colors.SUCCESS}
        borderWidth={2}
        borderColor={Colors.SUCCESS}
        borderPosition={BorderPosition.Inside}
      >
        <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
          <Text content="v" fontSize={12} color={Colors.TEXT_WHITE} />
        </Row>
      </Container>
    );
  }
  return (
    <Container
      width={20}
      height={20}
      borderRadius={6}
      borderWidth={2}
      borderColor={Colors.BORDER}
      borderPosition={BorderPosition.Inside}
    />
  );
}

function TodoItemRow(props: {
  todo: Todo;
  isSelected: boolean;
  onToggle: () => void;
  onRemove: () => void;
  onSelect: () => void;
}) {
  const { todo, isSelected } = props;
  return (
    <Container
      borderRadius={8}
      padding={12}
      borderWidth={1}
      borderColor={isSelected ? Colors.PRIMARY : Colors.BORDER}
      borderPosition={BorderPosition.Inside}
      color={isSelected ? Colors.PRIMARY_LIGHT : Colors.BG_CARD}
    >
        <Row mainAlignment={MainAxisAlignment.SpaceBetween} crossAlignment={CrossAxisAlignment.Center}>
        <Row crossAlignment={CrossAxisAlignment.Center} mainAxisSize={MainAxisSize.Min}>
          <PointerInteract onClick={props.onToggle} child={<Checkbox checked={todo.done} />} />
          <SizedBox width={12} />
          <PointerInteract onClick={props.onSelect} child={
            <Text
              content={todo.text}
              fontSize={14}
              color={todo.done ? Colors.TEXT_MUTED : Colors.TEXT_PRIMARY}
            />
          } />
        </Row>
        <PointerInteract onClick={props.onRemove} child={
          <Container width={28} height={28} borderRadius={6} color={Colors.DANGER_LIGHT}>
            <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
              <Text content="x" fontSize={16} color={Colors.DANGER} />
            </Row>
          </Container>
        } />
      </Row>
    </Container>
  );
}

function DetailPanel(props: { todo: Todo; onClose: () => void }) {
  const { todo } = props;
  return (
    <Container
      width={280}
      borderRadius={12}
      padding={20}
      color={Colors.BG_CARD}
      shadowColor={Colors.SHADOW}
      shadowOffset={[0, 4]}
      shadowBlur={16}
    >
      <Column crossAlignment={CrossAxisAlignment.Start}>
      <Row mainAlignment={MainAxisAlignment.SpaceBetween} crossAlignment={CrossAxisAlignment.Center}>
          <Text content={todo.text} fontSize={18} color={Colors.TEXT_PRIMARY} />
          <PointerInteract onClick={props.onClose} child={
            <Container width={28} height={28} borderRadius={6} color={Colors.PRIMARY_LIGHT}>
              <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
                <Text content="x" fontSize={14} color={Colors.TEXT_SECONDARY} />
              </Row>
            </Container>
          } />
        </Row>
        <SizedBox height={16} />
        <Container
          borderRadius={12}
          color={todo.done ? Colors.SUCCESS_LIGHT : Colors.PRIMARY_LIGHT}
          padding={8}
        >
          <Text
            content={todo.done ? "v Completed" : "o In Progress"}
            fontSize={12}
            color={todo.done ? Colors.SUCCESS : Colors.PRIMARY}
          />
        </Container>
        <SizedBox height={16} />
        <Text content="DESCRIPTION" fontSize={11} color={Colors.TEXT_MUTED} />
        <SizedBox height={4} />
        <Text content={todo.description || "No description"} fontSize={14} color={Colors.TEXT_SECONDARY} />
      </Column>
    </Container>
  );
}

function AddTaskModal(props: { onClose: () => void; onAdd: (text: string, description: string) => void }) {
  const [titleText, setTitleText] = useState("");
  const [descText, setDescText] = useState("");

  const titleController = new InputController({
    onInput: (text: string) => setTitleText(text),
  });

  const descController = new InputController({
    onInput: (text: string) => setDescText(text),
  });

  const handleAdd = () => {
    const trimmed = titleText.trim();
    if (!trimmed) return;
    props.onAdd(trimmed, descText);
    props.onClose();
  };

  return (
    <Positioned left={0} top={0} right={0} bottom={0}>
      <Container color={Colors.MODAL_BACKDROP}>
        <Column mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
          <Container
            width={420}
            borderRadius={12}
            padding={24}
            color={Colors.BG_CARD}
            shadowColor={Colors.SHADOW}
            shadowOffset={[0, 8]}
            shadowBlur={24}
          >
            <Column crossAlignment={CrossAxisAlignment.Start}>
              <Row mainAlignment={MainAxisAlignment.SpaceBetween} crossAlignment={CrossAxisAlignment.Center}>
                <Text content="Add New Task" fontSize={18} color={Colors.TEXT_PRIMARY} />
                <PointerInteract onClick={props.onClose} child={
                  <Container width={28} height={28} borderRadius={6} color={Colors.PRIMARY_LIGHT}>
                    <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
                      <Text content="x" fontSize={14} color={Colors.TEXT_SECONDARY} />
                    </Row>
                  </Container>
                } />
              </Row>
              <SizedBox height={24} />
              <Text content="Title" fontSize={12} color={Colors.TEXT_SECONDARY} />
              <SizedBox height={4} />
              <Container
                borderWidth={1}
                borderColor={Colors.BORDER}
                borderRadius={8}
                borderPosition={BorderPosition.Inside}
              >
                <Input
                  controller={titleController}
                  placeholder="Enter task title..."
                  fontSize={14}
                  width={372}
                  height={36}
                />
              </Container>
              <SizedBox height={16} />
              <Text content="Description" fontSize={12} color={Colors.TEXT_SECONDARY} />
              <SizedBox height={4} />
              <Container
                borderWidth={1}
                borderColor={Colors.BORDER}
                borderRadius={8}
                borderPosition={BorderPosition.Inside}
              >
                <Input
                  controller={descController}
                  placeholder="Enter description..."
                  fontSize={14}
                  width={372}
                  height={80}
                  multiline={true}
                />
              </Container>
              <SizedBox height={24} />
              <Row mainAlignment={MainAxisAlignment.End} crossAlignment={CrossAxisAlignment.Center}>
                <PointerInteract onClick={props.onClose} child={
                  <Container
                    height={36}
                    borderRadius={8}
                    borderWidth={1}
                    borderColor={Colors.BORDER}
                    borderPosition={BorderPosition.Inside}
                    padding={8}
                  >
                    <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
                      <Text content="Cancel" fontSize={14} color={Colors.TEXT_SECONDARY} />
                    </Row>
                  </Container>
                } />
                <SizedBox width={8} />
                <PointerInteract onClick={handleAdd} child={
                  <Container
                    height={36}
                    borderRadius={8}
                    color={Colors.PRIMARY}
                    padding={8}
                  >
                    <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
                      <Text content="Add Task" fontSize={14} color={Colors.TEXT_WHITE} />
                    </Row>
                  </Container>
                } />
              </Row>
            </Column>
          </Container>
        </Column>
      </Container>
    </Positioned>
  );
}

function TodoList() {
  const todos = useAtomValue(todosAtom);
  const toggleTodo = useSetAtom(toggleTodoAtom);
  const removeTodo = useSetAtom(removeTodoAtom);
  const addTodo = useSetAtom(addTodoAtom);
  const selectedTodoId = useAtomValue(selectedTodoIdAtom);
  const setSelectedTodoId = useSetAtom(selectedTodoIdAtom);

  const [showModal, setShowModal] = useState(false);

  const selectedTodo = todos.find((t) => t.id === selectedTodoId) ?? null;

  const handleSelect = (id: number) => {
    setSelectedTodoId(selectedTodoId === id ? null : id);
  };

  return (
    <Expanded>
      <Container color={Colors.BG_APP}>
        <Stack>
          <Column mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
            <Row crossAlignment={CrossAxisAlignment.Start} mainAlignment={MainAxisAlignment.Center}>
              <Container
                width={600}
                borderRadius={12}
                padding={24}
                color={Colors.BG_CARD}
                shadowColor={Colors.SHADOW}
                shadowOffset={[0, 4]}
                shadowBlur={16}
              >
                <Column crossAlignment={CrossAxisAlignment.Start} mainAxisSize={MainAxisSize.Min}>
                  <Row mainAlignment={MainAxisAlignment.SpaceBetween} crossAlignment={CrossAxisAlignment.Center}>
                    <Text content="My Tasks" fontSize={24} color={Colors.TEXT_PRIMARY} />
                    <PointerInteract onClick={() => setShowModal(true)} child={
                      <Container
                        height={36}
                        borderRadius={8}
                        color={Colors.PRIMARY}
                        padding={8}
                      >
                        <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center} mainAxisSize={MainAxisSize.Min}>
                          <Text content="+ New Task" fontSize={14} color={Colors.TEXT_WHITE} />
                        </Row>
                      </Container>
                    } />
                  </Row>
                  <SizedBox height={16} />
                  {todos.length === 0 && (
                    <Column crossAlignment={CrossAxisAlignment.Center}>
                      <SizedBox height={32} />
                      <Text content="No tasks yet" fontSize={16} color={Colors.TEXT_MUTED} />
                      <SizedBox height={8} />
                      <Text content='Click "+ New Task" to add one' fontSize={12} color={Colors.TEXT_MUTED} />
                      <SizedBox height={32} />
                    </Column>
                  )}
                  {todos.map((todo) => (
                    <Column key={todo.id} mainAxisSize={MainAxisSize.Min}>
                      <TodoItemRow
                        todo={todo}
                        isSelected={todo.id === selectedTodoId}
                        onToggle={() => toggleTodo(todo.id)}
                        onRemove={() => {
                          removeTodo(todo.id);
                          if (selectedTodoId === todo.id) setSelectedTodoId(null);
                        }}
                        onSelect={() => handleSelect(todo.id)}
                      />
                      <SizedBox height={8} />
                    </Column>
                  ))}
                </Column>
              </Container>
              {selectedTodo && (
                <>
                  <SizedBox width={24} />
                  <DetailPanel
                    todo={selectedTodo}
                    onClose={() => setSelectedTodoId(null)}
                  />
                </>
              )}
            </Row>
          </Column>
          {showModal && (
            <AddTaskModal
              onClose={() => setShowModal(false)}
              onAdd={(text, description) => addTodo({ text, description })}
            />
          )}
        </Stack>
      </Container>
    </Expanded>
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
