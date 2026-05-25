import { CrossAxisAlignment, Row, SizedBox } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function RowBasic() {
    return (
        <Row crossAlignment={CrossAxisAlignment.Start}>
            <SizedBox width={50} />
            <SizedBox width={30} />
        </Row>
    );
}

renderRoot(RowBasic);
