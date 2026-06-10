import { Alignment, Container, Text } from "@tur/react";
import { Color, renderRoot } from "@tur/react-renderer";

function ContainerShadow() {
    return (
        <Container
            width={200}
            height={200}
            color={Color.hex("#ffffff")}
            borderRadius={8}
            shadowColor={Color.rgba(0, 0, 0, 80)}
            shadowOffset={[4, 4]}
            shadowBlur={12}
            alignment={Alignment.Center}
        >
            <Text content="Shadow" fontSize={16} color={Color.hex("#333333")} />
        </Container>
    );
}

renderRoot(ContainerShadow);
