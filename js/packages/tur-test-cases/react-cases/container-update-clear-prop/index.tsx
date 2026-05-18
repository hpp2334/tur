import { useState } from "react";
import { renderRoot } from "@tur/react-renderer";
import { Container, PointerInteract } from "@tur/react";

function App() {
  const [checked, setChecked] = useState(true);

  return (
    <Container height={100} width={200} padding={20}>
      <PointerInteract onClick={() => setChecked(false)} child={
        checked ? (
          <Container
            width={40}
            height={40}
            borderRadius={8}
            color={{ r: 34, g: 197, b: 94, a: 255 }}
            borderWidth={2}
            borderColor={{ r: 34, g: 197, b: 94, a: 255 }}
            borderPosition={1}
          />
        ) : (
          <Container
            width={40}
            height={40}
            borderRadius={8}
            borderWidth={2}
            borderColor={{ r: 226, g: 232, b: 240, a: 255 }}
            borderPosition={1}
          />
        )
      } />
    </Container>
  );
}

renderRoot(App);
