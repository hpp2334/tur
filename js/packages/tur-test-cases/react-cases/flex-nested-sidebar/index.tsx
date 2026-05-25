import {
    Column,
    Container,
    CrossAxisAlignment,
    Row,
    SizedBox,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function FlexNestedSidebar() {
    return (
        <Row>
            <Container width={200}>
                <Column crossAlignment={CrossAxisAlignment.Start}>
                    <SizedBox height={40} />
                </Column>
            </Container>
            <Container>
                <Column crossAlignment={CrossAxisAlignment.Start}>
                    <SizedBox height={20} />
                </Column>
            </Container>
        </Row>
    );
}

renderRoot(FlexNestedSidebar);
