import { Column, Container, Row, Text, CrossAxisAlignment } from "@tur/react";

interface Tab {
  id: string;
  label: string;
}

export function Sidebar(props: { tabs: Tab[]; activeId: string }) {
  return (
    <Container color={"#1a1a2e" as unknown as import("@tur/react-renderer").Color} width={200}>
      <Column>
        {props.tabs.map((tab) => (
          <Container
            color={(tab.id === props.activeId ? "#0f3460" : "#16213e") as unknown as import("@tur/react-renderer").Color}
            padding={12}
            height={40}
          >
            <Row crossAlignment={CrossAxisAlignment.Center}>
              <Text content={tab.label} fontSize={14} />
            </Row>
          </Container>
        ))}
      </Column>
    </Container>
  );
}
