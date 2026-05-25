import {
    CrossAxisAlignment,
    MainAxisAlignment,
    Row,
    SizedBox,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function FlexRowMainCenter() {
    return (
        <Row
            mainAlignment={MainAxisAlignment.Center}
            crossAlignment={CrossAxisAlignment.Start}
        >
            <SizedBox width={50} />
            <SizedBox width={30} />
        </Row>
    );
}

renderRoot(FlexRowMainCenter);
