import {
    Alignment,
    AnimatedContainer,
    AnimatedPositioned,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    Expanded,
    MainAxisAlignment,
    PointerInteract,
    Row,
    SizedBox,
    Stack,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useState } from "react";

const CURVES = ["linear", "easeIn", "easeOut", "easeInOut"] as const;
const DURATIONS = [200, 400, 800, 1500] as const;
const DEMO_COLORS = [
    Color.rgb(59, 130, 246),
    Color.rgb(239, 68, 68),
    Color.rgb(34, 197, 94),
    Color.rgb(168, 85, 247),
] as const;

function AnimationPlayground() {
    const [curveIdx, setCurveIdx] = useState(2);
    const [durIdx, setDurIdx] = useState(1);
    const [toggled, setToggled] = useState(false);
    const [colorIdx, setColorIdx] = useState(0);
    const [posToggled, setPosToggled] = useState(false);
    const [infinite, setInfinite] = useState(false);

    const curve = CURVES[curveIdx];
    const duration = DURATIONS[durIdx];
    const targetColor = DEMO_COLORS[colorIdx];

    const handleToggleSize = () => {
        setToggled((t) => !t);
    };

    const handleTogglePos = () => {
        setPosToggled((t) => !t);
    };

    const handleColorCycle = () => {
        setColorIdx((c) => (c + 1) % DEMO_COLORS.length);
    };

    return (
        <Expanded>
            <Container color={Color.hex("#f8fafc")}>
                <Row crossAlignment={CrossAxisAlignment.Center}>
                    <Container width={12} />
                    <Expanded>
                        <Column
                            mainAlignment={MainAxisAlignment.Center}
                            crossAlignment={CrossAxisAlignment.Center}
                        >
                            <Stack>
                                <Container width={280} height={200} />
                                <AnimatedPositioned
                                    left={posToggled ? 200 : 20}
                                    top={posToggled ? 140 : 20}
                                    duration={duration}
                                    curve={curve}
                                    repeatCount={infinite ? 999999 : undefined}
                                >
                                    <AnimatedContainer
                                        width={toggled ? 120 : 60}
                                        height={toggled ? 120 : 60}
                                        borderRadius={toggled ? 60 : 12}
                                        color={targetColor}
                                        duration={duration}
                                        curve={curve}
                                        repeatCount={infinite ? 999999 : undefined}
                                    />
                                </AnimatedPositioned>
                            </Stack>

                            <SizedBox height={16} />

                            <Text content="CURVE" fontSize={10} color={Color.rgb(148, 163, 184)} />
                            <SizedBox height={4} />
                            <Row mainAlignment={MainAxisAlignment.Center}>
                                {CURVES.map((c, i) => (
                                    <PointerInteract
                                        key={c}
                                        onClick={() => setCurveIdx(i)}
                                        child={
                                            <Container
                                                width={70}
                                                height={30}
                                                borderRadius={6}
                                                color={i === curveIdx ? Color.rgb(59, 130, 246) : Color.rgb(226, 232, 240)}
                                                alignment={Alignment.Center}
                                            >
                                                <Text
                                                    content={c}
                                                    fontSize={11}
                                                    color={i === curveIdx ? Color.rgb(255, 255, 255) : Color.rgb(71, 85, 105)}
                                                />
                                            </Container>
                                        }
                                    />
                                ))}
                            </Row>

                            <SizedBox height={12} />

                            <Text content="DURATION (ms)" fontSize={10} color={Color.rgb(148, 163, 184)} />
                            <SizedBox height={4} />
                            <Row mainAlignment={MainAxisAlignment.Center}>
                                {DURATIONS.map((d, i) => (
                                    <PointerInteract
                                        key={d}
                                        onClick={() => setDurIdx(i)}
                                        child={
                                            <Container
                                                width={70}
                                                height={30}
                                                borderRadius={6}
                                                color={i === durIdx ? Color.rgb(59, 130, 246) : Color.rgb(226, 232, 240)}
                                                alignment={Alignment.Center}
                                            >
                                                <Text
                                                    content={String(d)}
                                                    fontSize={11}
                                                    color={i === durIdx ? Color.rgb(255, 255, 255) : Color.rgb(71, 85, 105)}
                                                />
                                            </Container>
                                        }
                                    />
                                ))}
                            </Row>

                            <SizedBox height={16} />

                            <Row mainAlignment={MainAxisAlignment.Center}>
                                <PointerInteract
                                    onClick={handleToggleSize}
                                    child={
                                        <Container
                                            width={70}
                                            height={30}
                                            borderRadius={6}
                                            color={toggled ? Color.rgb(59, 130, 246) : Color.rgb(226, 232, 240)}
                                            alignment={Alignment.Center}
                                        >
                                            <Text
                                                content="Size"
                                                fontSize={11}
                                                color={toggled ? Color.rgb(255, 255, 255) : Color.rgb(71, 85, 105)}
                                            />
                                        </Container>
                                    }
                                />
                                <SizedBox width={6} />
                                <PointerInteract
                                    onClick={handleColorCycle}
                                    child={
                                        <Container
                                            width={70}
                                            height={30}
                                            borderRadius={6}
                                            color={colorIdx > 0 ? Color.rgb(59, 130, 246) : Color.rgb(226, 232, 240)}
                                            alignment={Alignment.Center}
                                        >
                                            <Text
                                                content="Color"
                                                fontSize={11}
                                                color={colorIdx > 0 ? Color.rgb(255, 255, 255) : Color.rgb(71, 85, 105)}
                                            />
                                        </Container>
                                    }
                                />
                                <SizedBox width={6} />
                                <PointerInteract
                                    onClick={handleTogglePos}
                                    child={
                                        <Container
                                            width={70}
                                            height={30}
                                            borderRadius={6}
                                            color={posToggled ? Color.rgb(59, 130, 246) : Color.rgb(226, 232, 240)}
                                            alignment={Alignment.Center}
                                        >
                                            <Text
                                                content="Move"
                                                fontSize={11}
                                                color={posToggled ? Color.rgb(255, 255, 255) : Color.rgb(71, 85, 105)}
                                            />
                                        </Container>
                                    }
                                />
                            </Row>

                            <SizedBox height={12} />

                            <PointerInteract
                                onClick={() => setInfinite((v) => !v)}
                                child={
                                    <Container
                                        width={100}
                                        height={30}
                                        borderRadius={6}
                                        color={infinite ? Color.rgb(59, 130, 246) : Color.rgb(226, 232, 240)}
                                        alignment={Alignment.Center}
                                    >
                                        <Text
                                            content={infinite ? "Infinite ✓" : "Infinite"}
                                            fontSize={11}
                                            color={infinite ? Color.rgb(255, 255, 255) : Color.rgb(71, 85, 105)}
                                        />
                                    </Container>
                                }
                            />
                        </Column>
                    </Expanded>
                </Row>
            </Container>
        </Expanded>
    );
}

renderRoot(AnimationPlayground);
