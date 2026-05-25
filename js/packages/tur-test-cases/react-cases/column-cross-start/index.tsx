import { Column, CrossAxisAlignment, SizedBox } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ColumnCrossStart() {
    return (
        <Column crossAlignment={CrossAxisAlignment.Start}>
            <SizedBox width={100} height={50} />
        </Column>
    );
}

renderRoot(ColumnCrossStart);
