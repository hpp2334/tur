import { Column, Container, Expanded, PointerInteract, Row, SizedBox, Text, Color, MainAxisAlignment, CrossAxisAlignment } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useState } from "react";

function Counter() {
    const [count, setCount] = useState(0);

    return (
        <Expanded>
            <Container color={Color.hex("#f8fafc")}>
                <Column mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}>
                    <Container width={300} borderRadius={12} padding={24} color={Color.hex("#ffffff")}>
                        <Column crossAlignment={CrossAxisAlignment.Center}>
                            <Text content={"Count: " + count} queryKey={["count"]} fontSize={36} color={Color.hex("#1e293b")} />
                            <SizedBox height={20} />
                            <Row mainAlignment={MainAxisAlignment.Center}>
                                <PointerInteract
                                    onClick={() => setCount((n) => n + 1)}
                                    child={<Container width={100} height={44} borderRadius={8} color={Color.hex("#6366f1")}><Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}><Text content="+1" fontSize={18} color={Color.hex("#ffffff")} /></Row></Container>}
                                />
                                <SizedBox width={12} />
                                <PointerInteract
                                    onClick={() => setCount((n) => n - 1)}
                                    child={<Container width={100} height={44} borderRadius={8} color={Color.hex("#ef4444")}><Row mainAlignment={MainAxisAlignment.Center} crossAlignment={CrossAxisAlignment.Center}><Text content="-1" fontSize={18} color={Color.hex("#ffffff")} /></Row></Container>}
                                />
                            </Row>
                        </Column>
                    </Container>
                </Column>
            </Container>
        </Expanded>
    );
}

renderRoot(Counter);
