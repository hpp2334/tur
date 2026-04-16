import { createSignal } from "solid-js";
import { renderRoot } from "@tur/solidjs-renderer";
import {
  Column,
  Row,
  Text,
  Container,
  SizedBox,
  MainAxisAlignment,
  CrossAxisAlignment,
} from "@tur/solidjs";

function Counter() {
  const [count, _setCount] = createSignal(0);

  const label = () => {
    const n = count();
    if (n === 0) return "Zero";
    if (n > 0 && n <= 5) return "Low";
    if (n <= 10) return "Medium";
    return "High";
  };

  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="Counter" fontSize={28} />
        <SizedBox height={24} />
        <Container padding={16}>
          <Column crossAlignment={CrossAxisAlignment.Center}>
            <Text content={`${count()}`} fontSize={48} />
            <SizedBox height={4} />
            <Text content={label()} fontSize={14} />
          </Column>
        </Container>
        <SizedBox height={24} />
        <Row mainAlignment={MainAxisAlignment.Center}>
          <Container padding={8}>
            <Text content="  -  " fontSize={20} />
          </Container>
          <SizedBox width={16} />
          <Container padding={8}>
            <Text content="  +  " fontSize={20} />
          </Container>
        </Row>
        <SizedBox height={16} />
        <Row mainAlignment={MainAxisAlignment.Center}>
          <Text content="Steps:" fontSize={12} />
          <SizedBox width={4} />
          <Text content="-5  -1  +1  +5" fontSize={12} />
        </Row>
      </Column>
    </Container>
  );
}

renderRoot(Counter);
