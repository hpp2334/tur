import { CrossAxisAlignment, Expanded, Row, SizedBox } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function FlexRowExpanded() {
    return (
        <Row crossAlignment={CrossAxisAlignment.Start}>
            <SizedBox width={50} />
            <Expanded>
                <SizedBox />
            </Expanded>
        </Row>
    );
}

renderRoot(FlexRowExpanded);
