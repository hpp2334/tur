import { Column, Container, Row, Text, Color } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ContainerFlexSized() {
    return (
        <Column>
            <Row>
                <Container width={100} height={44} color={Color.hex("#6366f1")} queryKey={["btn"]}>
                    <Text content="Btn" fontSize={14} color={Color.hex("#ffffff")} />
                </Container>
            </Row>
        </Column>
    );
}

renderRoot(ContainerFlexSized);
