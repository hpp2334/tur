import {
    CrossAxisAlignment,
    MainAxisAlignment,
    Row,
    SizedBox,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function FlexRowMainEnd() {
    return (
        <Row
            mainAlignment={MainAxisAlignment.End}
            crossAlignment={CrossAxisAlignment.Start}
        >
            <SizedBox width={50} />
            <SizedBox width={30} />
        </Row>
    );
}

renderRoot(FlexRowMainEnd);
