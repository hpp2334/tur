import { renderRoot } from "@tur/solidjs-renderer";
import {
  Stack,
  Positioned,
  Text,
  Container,
  SizedBox,
  CrossAxisAlignment,
} from "@tur/solidjs";

function StackDemo() {
  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="Stack + Positioned" fontSize={18} />
        <SizedBox height={16} />
        <Stack>
          <Positioned left={0} top={0}>
            <Text content="Bottom-left" />
          </Positioned>
          <Positioned right={0} top={0}>
            <Text content="Bottom-right" />
          </Positioned>
          <Positioned left={40} top={20}>
            <Text content="Offset" />
          </Positioned>
        </Stack>

        <SizedBox height={24} />
        <Text content="Overlapping" fontSize={18} />
        <SizedBox height={8} />
        <Stack>
          <Positioned left={0} top={0}>
            <Text content="First" fontSize={24} />
          </Positioned>
          <Positioned left={30} top={10}>
            <Text content="Second" fontSize={24} />
          </Positioned>
          <Positioned left={60} top={20}>
            <Text content="Third" fontSize={24} />
          </Positioned>
        </Stack>
      </Column>
    </Container>
  );
}

renderRoot(StackDemo);
