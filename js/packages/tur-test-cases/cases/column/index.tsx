import { renderRoot } from "@tur/solidjs-renderer";
import {
  Column,
  Text,
  Container,
  SizedBox,
  CrossAxisAlignment,
  MainAxisAlignment,
} from "@tur/solidjs";

function ColumnDemo() {
  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="Column: Start" fontSize={18} />
        <SizedBox height={8} />
        <Column>
          <Text content="A" />
          <Text content="B" />
          <Text content="C" />
        </Column>

        <SizedBox height={16} />
        <Text content="Column: Center" fontSize={18} />
        <SizedBox height={8} />
        <Column mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
          <Text content="X" />
          <Text content="Y" />
          <Text content="Z" />
        </Column>

        <SizedBox height={16} />
        <Text content="Column: SpaceBetween" fontSize={18} />
        <SizedBox height={8} />
        <Column mainAlignment={MainAxisAlignment.SpaceBetween}>
          <Text content="One" />
          <Text content="Two" />
          <Text content="Three" />
        </Column>
      </Column>
    </Container>
  );
}

renderRoot(ColumnDemo);
