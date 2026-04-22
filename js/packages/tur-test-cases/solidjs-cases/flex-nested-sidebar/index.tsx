import { renderRoot } from "@tur/solidjs-renderer";
import { Row, Column, Container, SizedBox, CrossAxisAlignment } from "@tur/solidjs";

function FlexNestedSidebar() {
  return (
    <Row>
      <Container width={200}>
        <Column crossAlignment={CrossAxisAlignment.Start}>
          <SizedBox height={40} />
        </Column>
      </Container>
      <Container>
        <Column crossAlignment={CrossAxisAlignment.Start}>
          <SizedBox height={20} />
        </Column>
      </Container>
    </Row>
  );
}

renderRoot(FlexNestedSidebar);
