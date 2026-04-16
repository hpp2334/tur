import { createSignal } from "solid-js";
import { renderRoot } from "@tur/solidjs-renderer";
import {
  Column,
  Row,
  Text,
  Container,
  SizedBox,
  MainAxisAlignment,
} from "@tur/solidjs";

function Counter() {
  const [count, _setCount] = createSignal(0);

  return (
    <Container padding={16}>
      <Column crossAlignment={1}>
        <Text content="Counter" fontSize={24} />
        <SizedBox height={16} />
        <Row mainAlignment={MainAxisAlignment.Center}>
          <Text content="-" fontSize={20} />
          <SizedBox width={24} />
          <Text content={`${count()}`} fontSize={20} />
          <SizedBox width={24} />
          <Text content="+" fontSize={20} />
        </Row>
        <SizedBox height={8} />
        <Text content={count() === 0 ? "zero" : count() > 0 ? "positive" : "negative"} />
      </Column>
    </Container>
  );
}

renderRoot(Counter);
