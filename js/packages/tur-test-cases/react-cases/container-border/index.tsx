import { BorderPosition, Container, SizedBox } from "@tur/react";
import { Color, renderRoot } from "@tur/react-renderer";

function ContainerBorder() {
    return (
        <Container
            width={200}
            height={200}
            padding={16}
            color={Color.hex("#ffffff")}
            borderColor={Color.hex("#000000")}
            borderWidth={2}
            borderRadius={8}
            borderPosition={BorderPosition.Inside}
        >
            <SizedBox width={100} height={100} />
        </Container>
    );
}

renderRoot(ContainerBorder);
