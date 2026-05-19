import { renderRoot } from "@tur/react-renderer";
import { Container, Row, SizedBox, CrossAxisAlignment } from "@tur/react";

function ContainerPaddingOffset() {
  return (
    <Container height={100} width={200} padding={20}>
      <Row crossAlignment={CrossAxisAlignment.Start}>
        <SizedBox width={40} height={40} />
      </Row>
    </Container>
  );
}

renderRoot(ContainerPaddingOffset);
