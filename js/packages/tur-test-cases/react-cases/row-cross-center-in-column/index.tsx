import {
    Column,
    CrossAxisAlignment,
    MainAxisSize,
    Row,
    SizedBox,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function RowCrossCenterInColumn() {
    return (
        <Column crossAlignment={CrossAxisAlignment.Start}>
            <Row
                crossAlignment={CrossAxisAlignment.Center}
                mainAxisSize={MainAxisSize.Min}
            >
                <SizedBox width={20} height={20} />
                <SizedBox width={40} height={10} />
            </Row>
            <SizedBox height={30} />
            <SizedBox height={20} />
        </Column>
    );
}

renderRoot(RowCrossCenterInColumn);
