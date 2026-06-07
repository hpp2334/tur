import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    createScrollController,
    Expanded,
    Positioned,
    Row,
    ScrollView,
    SizedBox,
    Stack,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import React, { useState } from "react";

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

const ITEM_HEIGHT = 56;
const VIEWPORT_WIDTH = 400;
const HEADER_HEIGHT = 40;
const SCROLLBAR_WIDTH = 8;
const SCROLLBAR_THUMB_COLOR = Color.rgb(148, 163, 184);

function ScrollList() {
    const [scrollOffset, setScrollOffset] = useState(0);
    const [maxExtent, setMaxExtent] = useState(0);
    const [viewportDim, setViewportDim] = useState(1);

    const controller = createScrollController({
        onScroll: (info: {
            offset: number;
            maxExtent: number;
            viewportDimension: number;
        }) => {
            setScrollOffset(info.offset);
            setMaxExtent(info.maxExtent);
            setViewportDim(info.viewportDimension);
        },
    });

    const contentHeight = items.length * ITEM_HEIGHT;
    const trackHeight = viewportDim;
    const thumbHeight =
        maxExtent > 0
            ? Math.max(20, (viewportDim / contentHeight) * trackHeight)
            : trackHeight;
    const thumbTop =
        maxExtent > 0
            ? (scrollOffset / maxExtent) * (trackHeight - thumbHeight)
            : 0;

    return (
        <Column crossAlignment={CrossAxisAlignment.Start}>
            <Container
                width={VIEWPORT_WIDTH}
                height={HEADER_HEIGHT}
                color={Color.rgb(99, 102, 241)}
            >
                <Row>
                    <Expanded>
                        <Container height={HEADER_HEIGHT}>
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
                <Stack>
                    <ScrollView controller={controller}>
                        <Column crossAlignment={CrossAxisAlignment.Start}>
                            {items.map((item, i) => (
                                <Container
                                    key={item}
                                    width={VIEWPORT_WIDTH}
                                    height={ITEM_HEIGHT}
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
                                        <Container height={ITEM_HEIGHT}>
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
                    <Positioned right={4} top={thumbTop}>
                        <Container
                            width={SCROLLBAR_WIDTH}
                            height={thumbHeight}
                            color={SCROLLBAR_THUMB_COLOR}
                            borderRadius={4}
                        />
                    </Positioned>
                </Stack>
            </Expanded>
        </Column>
    );
}

renderRoot(ScrollList);
