import {
    Column,
    Container,
    CrossAxisAlignment,
    Row,
    ScrollView,
    SizedBox,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ScrollViewNested() {
    return (
        <Row>
            <SizedBox width={200} />
            <ScrollView queryKey={["outer-scroll"]}>
                <Column crossAlignment={CrossAxisAlignment.Start}>
                    <SizedBox height={100} />
                    <Container height={200} queryKey={["inner-wrapper"]}>
                        <ScrollView queryKey={["inner-scroll"]}>
                            <Column crossAlignment={CrossAxisAlignment.Start}>
                                <SizedBox height={200} />
                                <SizedBox height={200} />
                                <SizedBox height={200} />
                            </Column>
                        </ScrollView>
                    </Container>
                    <SizedBox height={400} />
                </Column>
            </ScrollView>
        </Row>
    );
}

renderRoot(ScrollViewNested);
