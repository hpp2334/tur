import { useState } from "react";
import { renderRoot } from "@tur/react-renderer";
import { Container, PointerInteract, Color } from "@tur/react";

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
            color={Color.rgba(34, 197, 94, 255)}
            borderWidth={2}
            borderColor={Color.rgba(34, 197, 94, 255)}
            borderPosition={1}
          />
        ) : (
          <Container
            width={40}
            height={40}
            borderRadius={8}
            borderWidth={2}
            borderColor={Color.rgba(226, 232, 240, 255)}
            borderPosition={1}
          />
        )
      } />
    </Container>
  );
}

renderRoot(App);
