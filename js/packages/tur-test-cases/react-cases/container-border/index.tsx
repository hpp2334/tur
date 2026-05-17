import { renderRoot } from "@tur/react-renderer";
import { Container, SizedBox, BorderPosition } from "@tur/react";

function ContainerBorder() {
  return (
    <Container
      width={200}
      height={200}
      padding={16}
      color="#ffffff"
      borderColor="#000000"
      borderWidth={2}
      borderRadius={8}
      borderPosition={BorderPosition.Inside}
    >
      <SizedBox width={100} height={100} />
    </Container>
  );
}

renderRoot(ContainerBorder);
