import { renderRoot } from "@tur/solidjs-renderer";
import {
  Column,
  Row,
  Expanded,
  Text,
  Container,
  SizedBox,
  CrossAxisAlignment,
} from "@tur/solidjs";

function ExpandedDemo() {
  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="Expanded / Flex" fontSize={24} />
        <SizedBox height={16} />

        <Text content="Equal flex in Row:" fontSize={14} />
        <SizedBox height={8} />
        <Row>
          <Expanded flex={1}>
            <Text content="flex=1" />
          </Expanded>
          <Expanded flex={1}>
            <Text content="flex=1" />
          </Expanded>
          <Expanded flex={1}>
            <Text content="flex=1" />
          </Expanded>
        </Row>

        <SizedBox height={16} />
        <Text content="Unequal flex in Row:" fontSize={14} />
        <SizedBox height={8} />
        <Row>
          <Expanded flex={1}>
            <Text content="1" />
          </Expanded>
          <Expanded flex={2}>
            <Text content="2" />
          </Expanded>
          <Expanded flex={3}>
            <Text content="3" />
          </Expanded>
        </Row>

        <SizedBox height={16} />
        <Text content="Expanded in Column:" fontSize={14} />
        <SizedBox height={8} />
        <Column>
          <Expanded flex={1}>
            <Text content="Top (flex=1)" />
          </Expanded>
          <Expanded flex={2}>
            <Text content="Middle (flex=2)" />
          </Expanded>
          <Expanded flex={1}>
            <Text content="Bottom (flex=1)" />
          </Expanded>
        </Column>
      </Column>
    </Container>
  );
}

renderRoot(ExpandedDemo);
