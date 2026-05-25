import { Container, SizedBox } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ContainerBasic() {
    return (
        <Container padding={16}>
            <SizedBox width={100} height={100} />
        </Container>
    );
}

renderRoot(ContainerBasic);
