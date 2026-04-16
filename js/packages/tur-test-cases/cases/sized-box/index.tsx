import { renderRoot } from "@tur/solidjs-renderer";
import {
  Column,
  Row,
  Text,
  Container,
  SizedBox,
  CrossAxisAlignment,
} from "@tur/solidjs";

function SizedBoxDemo() {
  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="SizedBox Spacing" fontSize={24} />
        <SizedBox height={16} />

        <Text content="Vertical spacing (height):" fontSize={14} />
        <Row>
          <Text content="A" />
          <SizedBox width={20} />
          <Text content="B" />
          <SizedBox width={40} />
          <Text content="C" />
        </Row>

        <SizedBox height={16} />
        <Text content="Fixed-size boxes:" fontSize={14} />
        <SizedBox height={8} />
        <Row>
          <SizedBox width={60} height={30}>
            <Text content="60x30" />
          </SizedBox>
          <SizedBox width={100} height={50}>
            <Text content="100x50" />
          </SizedBox>
          <SizedBox width={40} height={80}>
            <Text content="40x80" />
          </SizedBox>
        </Row>

        <SizedBox height={16} />
        <Text content="Staircase spacing:" fontSize={14} />
        <SizedBox height={8} />
        <Column>
          <Text content="Step 1" />
          <SizedBox height={4} />
          <Text content="Step 2" />
          <SizedBox height={8} />
          <Text content="Step 3" />
          <SizedBox height={16} />
          <Text content="Step 4" />
          <SizedBox height={32} />
          <Text content="Step 5" />
        </Column>
      </Column>
    </Container>
  );
}

renderRoot(SizedBoxDemo);
