import {
    Column,
    Container,
    CrossAxisAlignment,
    PointerInteract,
    Positioned,
    Row,
    SizedBox,
    Stack,
    Text,
} from "@tur/react";
import { Colors } from "./theme";

interface Tab {
    id: string;
    label: string;
}

export function Sidebar(props: { tabs: Tab[]; activeId: string }) {
    return (
        <Container color={Colors.BG_SIDEBAR} width={220}>
            <Column>
                <SizedBox height={24} />
                <Container padding={20}>
                    <Row crossAlignment={CrossAxisAlignment.Center}>
                        <Container
                            width={8}
                            height={8}
                            borderRadius={4}
                            color={Colors.PRIMARY}
                        />
                        <SizedBox width={10} />
                        <Text
                            content="Tur Todo"
                            fontSize={20}
                            color={Colors.TEXT_WHITE}
                        />
                    </Row>
                </Container>
                <SizedBox height={24} />
                <Container padding={20}>
                    <Text
                        content="NAVIGATION"
                        fontSize={11}
                        color={Colors.TEXT_MUTED}
                    />
                </Container>
                <SizedBox height={4} />
                {props.tabs.map((tab) => {
                    const isActive = tab.id === props.activeId;
                    return (
                        <Container key={tab.id} padding={4}>
                            <PointerInteract
                                child={
                                    <Stack>
                                        <Container
                                            height={40}
                                            borderRadius={8}
                                            color={
                                                isActive
                                                    ? Colors.BG_SIDEBAR_ACTIVE
                                                    : undefined
                                            }
                                            padding={10}
                                        >
                                            <Row
                                                crossAlignment={
                                                    CrossAxisAlignment.Center
                                                }
                                            >
                                                <SizedBox width={16} />
                                                <Text
                                                    content={tab.label}
                                                    fontSize={14}
                                                    color={
                                                        isActive
                                                            ? Colors.TEXT_WHITE
                                                            : Colors.TEXT_MUTED
                                                    }
                                                />
                                            </Row>
                                        </Container>
                                        {isActive && (
                                            <Positioned top={8} left={0}>
                                                <Container
                                                    width={4}
                                                    height={24}
                                                    borderRadius={2}
                                                    color={Colors.PRIMARY}
                                                />
                                            </Positioned>
                                        )}
                                    </Stack>
                                }
                            />
                        </Container>
                    );
                })}
            </Column>
        </Container>
    );
}
