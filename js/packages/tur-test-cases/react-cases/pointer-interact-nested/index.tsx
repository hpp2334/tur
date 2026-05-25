import {
    Column,
    Container,
    CrossAxisAlignment,
    MainAxisSize,
    PointerInteract,
    Row,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useState } from "react";

function NestedPointerInteract() {
    const [outerClicks, setOuterClicks] = useState(0);
    const [innerClicks, setInnerClicks] = useState(0);
    const [translucentOuterClicks, setTranslucentOuterClicks] = useState(0);
    const [translucentInnerClicks, setTranslucentInnerClicks] = useState(0);

    return (
        <Column
            crossAlignment={CrossAxisAlignment.Start}
            mainAxisSize={MainAxisSize.Min}
        >
            <PointerInteract
                onClick={() => setOuterClicks(outerClicks + 1)}
                child={
                    <Container
                        queryKey={["outer-opaque"]}
                        width={80}
                        height={40}
                    >
                        <Row>
                            <PointerInteract
                                onClick={() => setInnerClicks(innerClicks + 1)}
                                child={
                                    <Container
                                        queryKey={["inner-opaque"]}
                                        width={60}
                                        height={30}
                                    />
                                }
                            />
                        </Row>
                    </Container>
                }
            />
            <PointerInteract
                onClick={() =>
                    setTranslucentOuterClicks(translucentOuterClicks + 1)
                }
                child={
                    <Container
                        queryKey={["outer-translucent"]}
                        width={80}
                        height={40}
                    >
                        <Row>
                            <PointerInteract
                                behavior={1}
                                onClick={() =>
                                    setTranslucentInnerClicks(
                                        translucentInnerClicks + 1,
                                    )
                                }
                                child={
                                    <Container
                                        queryKey={["inner-translucent"]}
                                        width={60}
                                        height={30}
                                    />
                                }
                            />
                        </Row>
                    </Container>
                }
            />
            <Text
                content={`opaque:${outerClicks}/${innerClicks}`}
                queryKey={["result-opaque"]}
            />
            <Text
                content={`translucent:${translucentOuterClicks}/${translucentInnerClicks}`}
                queryKey={["result-translucent"]}
            />
        </Column>
    );
}

renderRoot(NestedPointerInteract);
