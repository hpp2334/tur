import { renderRoot } from "@tur/solidjs-renderer";
import {
  Column,
  Text,
  Container,
  SizedBox,
  CrossAxisAlignment,
} from "@tur/solidjs";

function TextDemo() {
  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Start}>
        <Text content="Text Variants" fontSize={24} />
        <SizedBox height={16} />

        <Text content="Default size" />
        <SizedBox height={4} />
        <Text content="Small" fontSize={10} />
        <SizedBox height={4} />
        <Text content="Medium" fontSize={16} />
        <SizedBox height={4} />
        <Text content="Large" fontSize={28} />
        <SizedBox height={4} />
        <Text content="Extra Large" fontSize={36} />

        <SizedBox height={16} />
        <Text content="Repeated text:" fontSize={18} />
        <SizedBox height={4} />
        <Text content="Hello World" />
        <SizedBox height={2} />
        <Text content="Hello World" />
        <SizedBox height={2} />
        <Text content="Hello World" />
      </Column>
    </Container>
  );
}

renderRoot(TextDemo);
