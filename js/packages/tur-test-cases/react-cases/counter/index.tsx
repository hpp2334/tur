import {
    Alignment,
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    Expanded,
    MainAxisAlignment,
    PointerInteract,
    Row,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useState } from "react";

function Counter() {
    const [count, setCount] = useState(0);

    return (
        <Expanded>
            <Container color={Color.hex("#f8fafc")}>
                <Column
                    mainAlignment={MainAxisAlignment.Center}
                    crossAlignment={CrossAxisAlignment.Center}
                >
                    <Text
                        content={`Count: ${count}`}
                        queryKey={["count"]}
                        fontSize={36}
                        color={Color.hex("#1e293b")}
                    />
                    <Row mainAlignment={MainAxisAlignment.Center}>
                        <PointerInteract
                            onClick={() => setCount((n) => n + 1)}
                            child={
                                <Container
                                    width={100}
                                    height={44}
                                    borderRadius={8}
                                    color={Color.hex("#6366f1")}
                                    alignment={Alignment.Center}
                                >
                                    <Text
                                        content="+1"
                                        fontSize={18}
                                        color={Color.hex("#ffffff")}
                                    />
                                </Container>
                            }
                        />
                    </Row>
                </Column>
            </Container>
        </Expanded>
    );
}

renderRoot(Counter);
