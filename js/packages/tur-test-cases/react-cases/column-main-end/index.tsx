import {
    Column,
    CrossAxisAlignment,
    MainAxisAlignment,
    SizedBox,
} from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ColumnMainEnd() {
    return (
        <Column
            mainAlignment={MainAxisAlignment.End}
            crossAlignment={CrossAxisAlignment.Start}
        >
            <SizedBox height={50} />
            <SizedBox height={30} />
        </Column>
    );
}

renderRoot(ColumnMainEnd);
