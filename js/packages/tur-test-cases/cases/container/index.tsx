import { renderRoot } from "@tur/solidjs-renderer";
import {
  Column,
  Text,
  Container,
  SizedBox,
  CrossAxisAlignment,
} from "@tur/solidjs";

function ContainerDemo() {
  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="Container Variants" fontSize={24} />
        <SizedBox height={16} />

        <Text content="No padding:" fontSize={14} />
        <Container>
          <Text content="Inside default container" />
        </Container>

        <SizedBox height={12} />
        <Text content="Padding 8:" fontSize={14} />
        <Container padding={8}>
          <Text content="8px padding" />
        </Container>

        <SizedBox height={12} />
        <Text content="Padding 24:" fontSize={14} />
        <Container padding={24}>
          <Text content="24px padding" />
        </Container>

        <SizedBox height={12} />
        <Text content="Nested containers:" fontSize={14} />
        <Container padding={16}>
          <Container padding={8}>
            <Text content="Deeply nested" />
          </Container>
        </Container>
      </Column>
    </Container>
  );
}

renderRoot(ContainerDemo);
