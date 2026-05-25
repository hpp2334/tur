import {
    Color,
    Column,
    Container,
    CrossAxisAlignment,
    PointerInteract,
    Text,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useState } from "react";

function PointerRegionText() {
    const [state, setState] = useState("idle");

    return (
        <Column crossAlignment={CrossAxisAlignment.Start}>
            <PointerInteract
                onPointerEnter={() => setState("entered")}
                onPointerExit={() => setState("exited")}
                child={
                    <Container
                        width={100}
                        height={50}
                        color={Color.hex("#cccccc")}
                    >
                        <Text content={state} queryKey={["region-text"]} />
                    </Container>
                }
            />
        </Column>
    );
}

renderRoot(PointerRegionText);
