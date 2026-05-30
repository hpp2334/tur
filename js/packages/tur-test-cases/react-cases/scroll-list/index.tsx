import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    Expanded,
    Row,
    ScrollView,
    SizedBox,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

const items = [
    "Design system architecture",
    "Implement rendering engine",
    "Build layout algorithm",
    "Add text rendering support",
    "Create gesture handling",
    "Implement scroll views",
    "Add image support",
    "Build input components",
    "Write integration tests",
    "Deploy to production",
    "Refactor element traits",
    "Add keyboard navigation",
    "Build focus management",
    "Implement text selection",
    "Add composition/IME support",
    "Optimize layout performance",
    "Add accessibility features",
    "Build theme system",
    "Create animation framework",
    "Write API documentation",
];

const colors = [
    Color.rgb(239, 68, 68),
    Color.rgb(249, 115, 22),
    Color.rgb(234, 179, 8),
    Color.rgb(34, 197, 94),
    Color.rgb(6, 182, 212),
    Color.rgb(59, 130, 246),
    Color.rgb(139, 92, 246),
    Color.rgb(236, 72, 153),
    Color.rgb(168, 85, 247),
    Color.rgb(20, 184, 166),
    Color.rgb(251, 146, 60),
    Color.rgb(163, 230, 53),
    Color.rgb(56, 189, 248),
    Color.rgb(167, 139, 250),
    Color.rgb(244, 114, 182),
    Color.rgb(52, 211, 153),
    Color.rgb(251, 191, 36),
    Color.rgb(129, 140, 248),
    Color.rgb(248, 113, 113),
    Color.rgb(45, 212, 191),
];

function ScrollList() {
    return (
        <Column crossAlignment={CrossAxisAlignment.Start}>
            <Container width={400} height={40} color={Color.rgb(99, 102, 241)}>
                <Row>
                    <Expanded>
                        <Container height={40}>
                            <Text
                                content="Scroll List Demo"
                                fontSize={16}
                                color={Color.rgb(255, 255, 255)}
                            />
                        </Container>
                    </Expanded>
                </Row>
            </Container>
            <Expanded>
                <ScrollView>
                    <Column crossAlignment={CrossAxisAlignment.Start}>
                        {items.map((item, i) => (
                            <Container
                                key={item}
                                width={400}
                                height={56}
                                color={
                                    i % 2 === 0
                                        ? Color.rgb(30, 41, 59)
                                        : Color.rgb(15, 23, 42)
                                }
                            >
                                <Row>
                                    <SizedBox width={12} />
                                    <Container
                                        width={32}
                                        height={32}
                                        color={colors[i]}
                                        borderRadius={16}
                                    />
                                    <SizedBox width={12} />
                                    <Container height={56}>
                                        <Text
                                            content={item}
                                            fontSize={14}
                                            color={Color.rgb(226, 232, 240)}
                                        />
                                    </Container>
                                </Row>
                            </Container>
                        ))}
                    </Column>
                </ScrollView>
            </Expanded>
        </Column>
    );
}

renderRoot(ScrollList);
