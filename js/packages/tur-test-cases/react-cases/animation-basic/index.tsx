import {
    AnimatedContainer,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    Expanded,
    MainAxisAlignment,
    PointerInteract,
    Row,
    SizedBox,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useState } from "react";

function AnimationBasic() {
    const [size, setSize] = useState(60);

    return (
        <Expanded>
            <Column
                mainAlignment={MainAxisAlignment.Center}
                crossAlignment={CrossAxisAlignment.Center}
            >
                <AnimatedContainer
                    width={size}
                    height={size}
                    color={Color.rgb(59, 130, 246)}
                    borderRadius={12}
                    duration={600}
                    curve="easeInOut"
                />
                <SizedBox height={20} />
                <PointerInteract
                    onClick={() => setSize((s) => (s === 60 ? 200 : 60))}
                    child={
                        <Container
                            width={120}
                            height={40}
                            color={Color.rgb(34, 197, 94)}
                            borderRadius={8}
                        >
                            <Row
                                mainAlignment={MainAxisAlignment.Center}
                                crossAlignment={CrossAxisAlignment.Center}
                            >
                                <Text
                                    content="Animate"
                                    fontSize={14}
                                    color={Color.rgb(255, 255, 255)}
                                />
                            </Row>
                        </Container>
                    }
                />
            </Column>
        </Expanded>
    );
}

renderRoot(AnimationBasic);
