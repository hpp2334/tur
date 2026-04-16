import { renderRoot } from "@tur/solidjs-renderer";
import { Container, SizedBox } from "@tur/solidjs";

function ContainerBasic() {
  return (
    <Container padding={16}>
      <SizedBox width={100} height={100} />
    </Container>
  );
}

renderRoot(ContainerBasic);
