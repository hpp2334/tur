import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    Expanded,
    LazyColumn,
    LazyRow,
    PointerInteract,
    Row,
    SizedBox,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import React, { useState } from "react";

const ITEM_HEIGHT = 56;
const ITEM_WIDTH = 100;
const ITEM_HEIGHT_ROW = 140;
const ITEM_COUNT = 500;
const VIEWPORT_WIDTH = 400;

const accentColors = [
    Color.rgb(239, 68, 68),
    Color.rgb(249, 115, 22),
    Color.rgb(234, 179, 8),
    Color.rgb(34, 197, 94),
    Color.rgb(6, 182, 212),
    Color.rgb(59, 130, 246),
    Color.rgb(139, 92, 246),
    Color.rgb(236, 72, 153),
];

function TabButton({
    label,
    active,
    onClick,
}: {
    label: string;
    active: boolean;
    onClick: () => void;
}) {
    return (
        <PointerInteract
            onClick={onClick}
            child={
                <Container
                    width={80}
                    height={32}
                    color={
                        active ? Color.rgb(99, 102, 241) : Color.rgb(30, 41, 59)
                    }
                    borderRadius={6}
                >
                    <Container height={32}>
                        <Text
                            content={label}
                            fontSize={13}
                            color={
                                active
                                    ? Color.rgb(255, 255, 255)
                                    : Color.rgb(148, 163, 184)
                            }
                        />
                    </Container>
                </Container>
            }
        />
    );
}

function LazyColumnView() {
    return (
        <LazyColumn
            itemCount={ITEM_COUNT}
            overscan={5}
            renderItem={(index) => (
                <Container
                    width={VIEWPORT_WIDTH}
                    height={ITEM_HEIGHT}
                    color={
                        index % 2 === 0
                            ? Color.rgb(30, 41, 59)
                            : Color.rgb(15, 23, 42)
                    }
                >
                    <Row>
                        <SizedBox width={12} />
                        <Container
                            width={32}
                            height={32}
                            color={accentColors[index % accentColors.length]}
                            borderRadius={16}
                        />
                        <SizedBox width={12} />
                        <Container height={ITEM_HEIGHT}>
                            <Text
                                content={`Item #${index + 1}`}
                                fontSize={14}
                                color={Color.rgb(226, 232, 240)}
                            />
                        </Container>
                    </Row>
                </Container>
            )}
        />
    );
}

function LazyRowView() {
    return (
        <LazyRow
            itemCount={ITEM_COUNT}
            overscan={3}
            renderItem={(index) => (
                <Container
                    width={ITEM_WIDTH}
                    height={ITEM_HEIGHT_ROW}
                    color={accentColors[index % accentColors.length]}
                    borderRadius={8}
                >
                    <Column>
                        <SizedBox height={12} />
                        <Container
                            width={40}
                            height={40}
                            color={Color.rgb(255, 255, 255)}
                            borderRadius={20}
                        />
                        <SizedBox height={6} />
                        <Container height={20}>
                            <Text
                                content={`#${index + 1}`}
                                fontSize={12}
                                color={Color.rgb(255, 255, 255)}
                            />
                        </Container>
                    </Column>
                </Container>
            )}
        />
    );
}

function LazyListDemo() {
    const [mode, setMode] = useState<"column" | "row">("column");

    return (
        <Column crossAlignment={CrossAxisAlignment.Start}>
            <Row>
                <SizedBox width={12} />
                <TabButton
                    label="Column"
                    active={mode === "column"}
                    onClick={() => setMode("column")}
                />
                <SizedBox width={8} />
                <TabButton
                    label="Row"
                    active={mode === "row"}
                    onClick={() => setMode("row")}
                />
            </Row>
            <SizedBox height={8} />
            <Expanded>
                {mode === "column" ? <LazyColumnView /> : <LazyRowView />}
            </Expanded>
        </Column>
    );
}

renderRoot(LazyListDemo);
