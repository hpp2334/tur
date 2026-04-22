import { For } from "solid-js";
import { Column, Container, Text } from "@tur/solidjs";

interface Tab {
  id: string;
  label: string;
}

export function Sidebar(props: { tabs: Tab[]; activeId: string }) {
  return (
    <Container color={"#1a1a2e" as unknown as import("@tur/solidjs-renderer").Color} width={200}>
      <Column>
        <For each={props.tabs}>
          {(tab) => (
            <Container
              color={(tab.id === props.activeId ? "#0f3460" : "#16213e") as unknown as import("@tur/solidjs-renderer").Color}
              padding={12}
            >
              <Text content={tab.label} fontSize={14} />
            </Container>
          )}
        </For>
      </Column>
    </Container>
  );
}
