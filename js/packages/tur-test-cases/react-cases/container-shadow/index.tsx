import { renderRoot, Color } from "@tur/react-renderer";
import { Container, SizedBox } from "@tur/react";

function ContainerShadow() {
  return (
    <Container
      width={200}
      height={200}
      color={Color.hex("#ffffff")}
      borderRadius={8}
      shadowColor={Color.rgba(0, 0, 0, 80)}
      shadowOffset={[4, 4]}
      shadowBlur={12}
    >
      <SizedBox width={100} height={100} />
    </Container>
  );
}

renderRoot(ContainerShadow);
