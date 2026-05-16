import { renderRoot } from "@tur/react-renderer";
import {
  Column,
  Row,
  Text,
  Container,
  SizedBox,
  MainAxisAlignment,
  CrossAxisAlignment,
} from "@tur/react";

const TABS = [{ id: "todolist", label: "TodoList" }];

function Sidebar(props: { tabs: typeof TABS; activeId: string }) {
  return (
    <Container color={"#1a1a2e" as unknown as import("@tur/react-renderer").Color} width={200}>
      <Column>
        {props.tabs.map((tab) => (
          <Container
            color={(tab.id === props.activeId ? "#0f3460" : "#16213e") as unknown as import("@tur/react-renderer").Color}
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
