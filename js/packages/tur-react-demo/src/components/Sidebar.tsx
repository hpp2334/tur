import { Column, Container, Row, Text, CrossAxisAlignment } from "@tur/react";
import { Color } from "@tur/react-renderer";

interface Tab {
  id: string;
  label: string;
}

export function Sidebar(props: { tabs: Tab[]; activeId: string }) {
  return (
    <Container color={Color.hex("#1a1a2e")} width={200}>
      <Column>
        {props.tabs.map((tab) => (
          <Container
            key={tab.id}
            color={Color.hex(tab.id === props.activeId ? "#0f3460" : "#16213e")}
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
