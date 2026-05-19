import { renderRoot } from "@tur/react-renderer";
import { Row, Container, CrossAxisAlignment, SizedBox } from "@tur/react";

function RowCrossCenter() {
  return (
    <Container height={36} width={200}>
      <Row crossAlignment={CrossAxisAlignment.Center}>
        <SizedBox width={20} height={20} />
        <SizedBox width={40} height={10} />
      </Row>
    </Container>
  );
}

renderRoot(RowCrossCenter);
