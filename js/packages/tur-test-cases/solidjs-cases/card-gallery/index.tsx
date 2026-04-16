import { For } from "solid-js";
import { renderRoot } from "@tur/solidjs-renderer";
import {
  Column,
  Row,
  Text,
  Container,
  SizedBox,
  Stack,
  Positioned,
  CrossAxisAlignment,
  MainAxisAlignment,
} from "@tur/solidjs";

interface Card {
  readonly id: number;
  readonly title: string;
  readonly subtitle: string;
  readonly status: "active" | "draft" | "archived";
}

const CARDS: readonly Card[] = [
  { id: 1, title: "Project Alpha", subtitle: "Mobile app redesign", status: "active" },
  { id: 2, title: "Project Beta", subtitle: "Backend migration", status: "active" },
  { id: 3, title: "Project Gamma", subtitle: "Design system v2", status: "draft" },
  { id: 4, title: "Project Delta", subtitle: "Analytics dashboard", status: "active" },
  { id: 5, title: "Project Epsilon", subtitle: "Legacy cleanup", status: "archived" },
  { id: 6, title: "Project Zeta", subtitle: "API gateway", status: "draft" },
];

const STATUS_LABEL: Record<Card["status"], string> = {
  active: "\u25CF Active",
  draft: "\u25CB Draft",
  archived: "\u2014 Archived",
};

function CardItem(props: { card: Card }) {
  return (
    <Container padding={12}>
      <Row mainAlignment={MainAxisAlignment.SpaceBetween}>
        <Column>
          <Text content={props.card.title} fontSize={16} />
          <SizedBox height={2} />
          <Text content={props.card.subtitle} fontSize={12} />
        </Column>
        <Stack>
          <Positioned right={0} top={0}>
            <Text content={STATUS_LABEL[props.card.status]} fontSize={12} />
          </Positioned>
        </Stack>
      </Row>
    </Container>
  );
}

function CardGallery() {
  return (
    <Container padding={16}>
      <Column crossAlignment={CrossAxisAlignment.Center}>
        <Text content="Projects" fontSize={28} />
        <SizedBox height={4} />
        <Text content={`${CARDS.length} total`} fontSize={14} />
        <SizedBox height={16} />
        <Column>
          <For each={CARDS}>
            {(card) => (
              <>
                <CardItem card={card} />
                <SizedBox height={8} />
              </>
            )}
          </For>
        </Column>
      </Column>
    </Container>
  );
}

renderRoot(CardGallery);
