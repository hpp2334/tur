import { renderRoot } from "@tur/react-renderer";
import type { Color } from "@tur/react-renderer";
import { Container, SizedBox, BorderPosition } from "@tur/react";

function ContainerBorder() {
  return (
    <Container
      width={200}
      height={200}
      padding={16}
      color={"#ffffff" as unknown as Color}
      borderColor={"#000000" as unknown as Color}
      borderWidth={2}
      borderRadius={8}
      borderPosition={BorderPosition.Inside}
    >
      <SizedBox width={100} height={100} />
    </Container>
  );
}

renderRoot(ContainerBorder);
