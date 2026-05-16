import { renderRoot } from "@tur/react-renderer";
import {
  Column,
  Row,
  Expanded,
  Text,
  Container,
  SizedBox,
  CrossAxisAlignment,
  MainAxisAlignment,
} from "@tur/react";

interface Stat {
  readonly label: string;
  readonly value: string;
  readonly change: string;
}

interface Activity {
  readonly user: string;
  readonly action: string;
  readonly time: string;
}

const STATS: readonly Stat[] = [
  { label: "Revenue", value: "$12,480", change: "+12%" },
  { label: "Users", value: "1,842", change: "+8%" },
  { label: "Orders", value: "364", change: "-3%" },
];

const ACTIVITIES: readonly Activity[] = [
  { user: "Alice", action: "created a new project", time: "2m ago" },
  { user: "Bob", action: "completed task #142", time: "15m ago" },
  { user: "Carol", action: "commented on issue #89", time: "1h ago" },
  { user: "Dave", action: "deployed v2.4.1", time: "2h ago" },
  { user: "Eve", action: "merged PR #307", time: "3h ago" },
  { user: "Frank", action: "updated billing info", time: "5h ago" },
];

function StatCard(props: { stat: Stat }) {
  return (
    <Expanded flex={1}>
      <Container padding={12}>
        <Column crossAlignment={CrossAxisAlignment.Center}>
          <Text content={props.stat.value} fontSize={20} />
          <SizedBox height={2} />
          <Text content={props.stat.label} fontSize={12} />
          <SizedBox height={2} />
          <Text content={props.stat.change} fontSize={10} />
        </Column>
      </Container>
    </Expanded>
  );
}

function ActivityRow(props: { item: Activity }) {
  return (
    <Row mainAlignment={MainAxisAlignment.SpaceBetween}>
      <Row>
        <Text content={props.item.user} />
        <SizedBox width={4} />
        <Text content={props.item.action} />
      </Row>
      <Text content={props.item.time} fontSize={12} />
    </Row>
  );
}

function Dashboard() {
  return (
    <Container padding={16}>
      <Column>
        <Text content="Dashboard" fontSize={28} />
        <SizedBox height={16} />

        <Row>
          {STATS.map((stat) => (
            <>
              <StatCard stat={stat} />
              <SizedBox width={8} />
            </>
          ))}
        </Row>

        <SizedBox height={24} />
        <Text content="Recent Activity" fontSize={18} />
        <SizedBox height={12} />
        <Column>
          {ACTIVITIES.map((item) => (
            <>
              <ActivityRow item={item} />
              <SizedBox height={8} />
            </>
          ))}
        </Column>
      </Column>
    </Container>
  );
}

renderRoot(Dashboard);
