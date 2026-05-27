import {
    BorderPosition,
    Column,
    Container,
    CrossAxisAlignment,
    Expanded,
    Input,
    MainAxisAlignment,
    MainAxisSize,
    PointerInteract,
    Positioned,
    Row,
    SizedBox,
    Stack,
    Text,
} from "@tur/react";
import { renderRoot, createTextEditingController } from "@tur/react-renderer";
import { useAtomValue, useSetAtom } from "jotai/react";
import { atom, getDefaultStore } from "jotai";

interface Todo {
    readonly id: number;
    readonly text: string;
    readonly done: boolean;
    readonly description: string;
}

const INITIAL_TODOS: readonly Todo[] = [
    { id: 1, text: "Learn Rust", done: true, description: "Complete the Rust book and build a CLI tool" },
    { id: 2, text: "Build tur engine", done: false, description: "Implement the rendering engine with winit and vello" },
    { id: 3, text: "Write documentation", done: false, description: "Document the architecture and API surface" },
    { id: 4, text: "Ship v0.1.0", done: false, description: "First public release with core features" },
];

const { Color } = globalThis.TurReactRenderer as typeof import("@tur/react-renderer");

const Colors = {
    BG_APP: Color.hex("#f8fafc"),
    BG_CARD: Color.hex("#ffffff"),
    PRIMARY: Color.hex("#6366f1"),
    PRIMARY_DARK: Color.hex("#4f46e5"),
    PRIMARY_LIGHT: Color.hex("#eef2ff"),
    TEXT_PRIMARY: Color.hex("#1e293b"),
    TEXT_SECONDARY: Color.hex("#64748b"),
    TEXT_MUTED: Color.hex("#94a3b8"),
    TEXT_WHITE: Color.hex("#ffffff"),
    SUCCESS: Color.hex("#22c55e"),
    SUCCESS_LIGHT: Color.hex("#f0fdf4"),
    DANGER: Color.hex("#ef4444"),
    DANGER_LIGHT: Color.hex("#fef2f2"),
    BORDER: Color.hex("#e2e8f0"),
    SHADOW: Color.rgba(0, 0, 0, 60),
    MODAL_BACKDROP: Color.rgba(0, 0, 0, 102),
};

const store = getDefaultStore();

const todosAtom = atom<readonly Todo[]>(INITIAL_TODOS);

const addTodoAtom = atom(null, (_get: never, set: (a: never, v: never) => void, payload: { text: string; description: string }) => {
    const trimmed = payload.text.trim();
    if (!trimmed) return;
    set(todosAtom as never, (prev: readonly Todo[]) => [
        ...prev,
        { id: Date.now(), text: trimmed, description: payload.description, done: false },
    ] as readonly Todo[]);
});

const toggleTodoAtom = atom(null, (_get: never, set: (a: never, v: never) => void, id: number) => {
    set(todosAtom as never, (prev: readonly Todo[]) =>
        prev.map((t) => (t.id === id ? { ...t, done: !t.done } : t)),
    );
});

const removeTodoAtom = atom(null, (_get: never, set: (a: never, v: never) => void, id: number) => {
    set(todosAtom as never, (prev: readonly Todo[]) => prev.filter((t) => t.id !== id));
});

const selectedTodoIdAtom = atom<number | null>(null);
const showModalAtom = atom(false);
const titleTextAtom = atom("");
const descTextAtom = atom("");

const titleControllerAtom = atom(
    createTextEditingController({ onInput: (text: string) => store.set(titleTextAtom, text) }),
);
const descControllerAtom = atom(
    createTextEditingController({ onInput: (text: string) => store.set(descTextAtom, text) }),
);

function Checkbox(props: { checked: boolean }) {
    if (props.checked) {
        return (
            <Container width={20} height={20} borderRadius={6} color={Colors.SUCCESS} borderWidth={2} borderColor={Colors.SUCCESS} borderPosition={BorderPosition.Inside}>
                <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
                    <Text content="v" fontSize={12} color={Colors.TEXT_WHITE} />
                </Row>
            </Container>
        );
    }
    return <Container width={20} height={20} borderRadius={6} borderWidth={2} borderColor={Colors.BORDER} borderPosition={BorderPosition.Inside} />;
}

function TodoItemRow(props: { todo: Todo; isSelected: boolean; onToggle: () => void; onRemove: () => void; onSelect: () => void }) {
    const { todo, isSelected } = props;
    return (
        <PointerInteract onClick={props.onSelect} child={
            <Container borderRadius={8} padding={12} borderWidth={1} borderColor={isSelected ? Colors.PRIMARY : Colors.BORDER} borderPosition={BorderPosition.Inside} color={isSelected ? Colors.PRIMARY_LIGHT : Colors.BG_CARD}>
                <Row mainAlignment={MainAxisAlignment.SpaceBetween} crossAlignment={CrossAxisAlignment.Center}>
                    <Row crossAlignment={CrossAxisAlignment.Center} mainAxisSize={MainAxisSize.Min}>
                        <PointerInteract onClick={props.onToggle} child={<Checkbox checked={todo.done} />} />
                        <SizedBox width={12} />
                        <Text content={todo.text} fontSize={14} color={todo.done ? Colors.TEXT_MUTED : Colors.TEXT_PRIMARY} />
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
        } />
    );
}

function DetailPanel(props: { todo: Todo; onClose: () => void }) {
    const { todo } = props;
    return (
        <Container width={280} borderRadius={12} padding={20} color={Colors.BG_CARD} shadowColor={Colors.SHADOW} shadowOffset={[0, 4]} shadowBlur={16}>
            <Column mainAxisSize={MainAxisSize.Min} crossAlignment={CrossAxisAlignment.Start}>
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
                <Container borderRadius={12} color={todo.done ? Colors.SUCCESS_LIGHT : Colors.PRIMARY_LIGHT} padding={8}>
                    <Text content={todo.done ? "v Completed" : "o In Progress"} fontSize={12} color={todo.done ? Colors.SUCCESS : Colors.PRIMARY} />
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
    const titleText = useAtomValue(titleTextAtom);
    const descText = useAtomValue(descTextAtom);
    const titleController = useAtomValue(titleControllerAtom);
    const descController = useAtomValue(descControllerAtom);

    const trimmed = titleText.trim();
    const canAdd = trimmed.length > 0;

    const handleAdd = () => {
        if (!canAdd) return;
        props.onAdd(trimmed, descText);
        titleController.clear();
        descController.clear();
        store.set(titleTextAtom, "");
        store.set(descTextAtom, "");
        props.onClose();
    };

    return (
        <Positioned left={0} top={0} right={0} bottom={0}>
            <Container color={Colors.MODAL_BACKDROP}>
                <Column mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
                    <Container width={420} borderRadius={12} padding={24} color={Colors.BG_CARD} shadowColor={Colors.SHADOW} shadowOffset={[0, 8]} shadowBlur={24}>
                        <Column mainAxisSize={MainAxisSize.Min} crossAlignment={CrossAxisAlignment.Start}>
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
                            <Container borderWidth={1} borderColor={Colors.BORDER} borderRadius={8} borderPosition={BorderPosition.Inside}>
                                <Input controller={titleController} placeholder="Enter task title..." fontSize={14} width={372} height={36} />
                            </Container>
                            <SizedBox height={16} />
                            <Text content="Description" fontSize={12} color={Colors.TEXT_SECONDARY} />
                            <SizedBox height={4} />
                            <Container borderWidth={1} borderColor={Colors.BORDER} borderRadius={8} borderPosition={BorderPosition.Inside}>
                                <Input controller={descController} placeholder="Enter description..." fontSize={14} width={372} height={80} multiline={true} />
                            </Container>
                            <SizedBox height={24} />
                            <Row mainAlignment={MainAxisAlignment.End} crossAlignment={CrossAxisAlignment.Center}>
                                <PointerInteract onClick={props.onClose} child={
                                    <Container height={36} borderRadius={8} borderWidth={1} borderColor={Colors.BORDER} borderPosition={BorderPosition.Inside} padding={8}>
                                        <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center} mainAxisSize={MainAxisSize.Min}>
                                            <Text content="Cancel" fontSize={14} color={Colors.TEXT_SECONDARY} />
                                        </Row>
                                    </Container>
                                } />
                                <SizedBox width={8} />
                                <PointerInteract onClick={handleAdd} child={
                                    <Container height={36} borderRadius={8} color={canAdd ? Colors.PRIMARY : Colors.BORDER} padding={8}>
                                        <Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center} mainAxisSize={MainAxisSize.Min}>
                                            <Text content="Add Task" fontSize={14} color={canAdd ? Colors.TEXT_WHITE : Colors.TEXT_MUTED} />
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
    const setShowModal = useSetAtom(showModalAtom);
    const showModal = useAtomValue(showModalAtom);
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
                            <Container width={600} borderRadius={12} padding={24} color={Colors.BG_CARD} shadowColor={Colors.SHADOW} shadowOffset={[0, 4]} shadowBlur={16}>
                                <Column crossAlignment={CrossAxisAlignment.Start} mainAxisSize={MainAxisSize.Min}>
                                    <Row mainAlignment={MainAxisAlignment.SpaceBetween} crossAlignment={CrossAxisAlignment.Center}>
                                        <Text content="My Tasks" fontSize={24} color={Colors.TEXT_PRIMARY} />
                                        <PointerInteract onClick={() => setShowModal(true)} child={
                                            <Container height={36} borderRadius={8} color={Colors.PRIMARY} padding={8}>
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
                                            <TodoItemRow todo={todo} isSelected={todo.id === selectedTodoId} onToggle={() => toggleTodo(todo.id)} onRemove={() => { removeTodo(todo.id); if (selectedTodoId === todo.id) setSelectedTodoId(null); }} onSelect={() => handleSelect(todo.id)} />
                                            <SizedBox height={8} />
                                        </Column>
                                    ))}
                                </Column>
                            </Container>
                            {selectedTodo && (
                                <>
                                    <SizedBox width={24} />
                                    <DetailPanel todo={selectedTodo} onClose={() => setSelectedTodoId(null)} />
                                </>
                            )}
                        </Row>
                    </Column>
                    {showModal && <AddTaskModal onClose={() => setShowModal(false)} onAdd={(text, description) => addTodo({ text, description })} />}
                </Stack>
            </Container>
        </Expanded>
    );
}

renderRoot(TodoList);
