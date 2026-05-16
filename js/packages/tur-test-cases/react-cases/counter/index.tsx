import { useState } from "react";
import { renderRoot } from "@tur/react-renderer";
import {
  Column,
  Text,
  Container,
  PointerInteract,
} from "@tur/react";

function Counter() {
  const [count, setCount] = useState(0);

  return (
    <Container>
      <Column>
        <PointerInteract onClick={() => setCount((n) => n + 1)} child={
          <Container>
            <Text content="+1" />
          </Container>
        } />
        <Text content={`Count: ${count}`} queryKey={["count"]} />
      </Column>
    </Container>
  );
}

renderRoot(Counter);
