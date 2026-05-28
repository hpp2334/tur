import { CrossAxisAlignment, Row, SizedBox } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function FlexRowCrossStretch() {
    return (
        <Row crossAlignment={CrossAxisAlignment.Stretch}>
            <SizedBox width={50} />
            <SizedBox width={30} />
        </Row>
    );
}

renderRoot(FlexRowCrossStretch);
