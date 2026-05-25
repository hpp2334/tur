import {
    Column,
    Container,
    CrossAxisAlignment,
    MainAxisAlignment,
    Row,
    SizedBox,
    Text,
} from "@tur/react";
import { Color, renderRoot } from "@tur/react-renderer";

const TABS = [{ id: "todolist", label: "TodoList" }];

function Sidebar(props: { tabs: typeof TABS; activeId: string }) {
    return (
        <Container color={Color.hex("#1a1a2e")} width={200}>
            <Column>
                {props.tabs.map((tab) => (
                    <Container
                        key={tab.id}
                        color={Color.hex(
                            tab.id === props.activeId ? "#0f3460" : "#16213e",
                        )}
                        padding={12}
                    >
                        <Text content={tab.label} fontSize={14} />
                    </Container>
                ))}
            </Column>
        </Container>
    );
}

function Content() {
    return (
        <Container padding={16}>
            <Column crossAlignment={CrossAxisAlignment.Center}>
                <Text content="Todo List" fontSize={24} />
                <SizedBox height={16} />
                <Column>
                    <Row mainAlignment={MainAxisAlignment.SpaceBetween}>
                        <Text content="Buy milk" />
                        <Text content="\u2713" />
                    </Row>
                </Column>
            </Column>
        </Container>
    );
}

function App() {
    return (
        <Row>
            <Sidebar tabs={TABS} activeId="todolist" />
            <Content />
        </Row>
    );
}

renderRoot(App);
