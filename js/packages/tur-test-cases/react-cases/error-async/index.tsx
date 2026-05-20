import { renderRoot } from "@tur/react-renderer";
import { useEffect, useState } from "react";
import { Container, Column, Text } from "@tur/react";

function App() {
  const [x, setX] = useState(0);
  useEffect(() => {
    setX(1);
    throw new Error("microtask error from useEffect");
  });
  return (
    <Container>
      <Column>
        <Text content={`count: ${x}`} />
      </Column>
    </Container>
  );
}

renderRoot(App);
