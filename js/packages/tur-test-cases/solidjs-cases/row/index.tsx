import { renderRoot } from "@tur/solidjs-renderer";
import {
  Row,
  Text,
  Container,
  SizedBox,
  MainAxisAlignment,
  CrossAxisAlignment,
} from "@tur/solidjs";

function RowDemo() {
  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="Row: Default" fontSize={18} />
        <SizedBox height={8} />
        <Row>
          <Text content="A" />
          <Text content="B" />
          <Text content="C" />
        </Row>

        <SizedBox height={16} />
        <Text content="Row: SpaceBetween" fontSize={18} />
        <SizedBox height={8} />
        <Row mainAlignment={MainAxisAlignment.SpaceBetween}>
          <Text content="Left" />
          <Text content="Center" />
          <Text content="Right" />
        </Row>

        <SizedBox height={16} />
        <Text content="Row: SpaceEvenly" fontSize={18} />
        <SizedBox height={8} />
        <Row mainAlignment={MainAxisAlignment.SpaceEvenly}>
          <Text content="1" />
          <Text content="2" />
          <Text content="3" />
          <Text content="4" />
        </Row>
      </Column>
    </Container>
  );
}

renderRoot(RowDemo);
