import { Column, CrossAxisAlignment, SizedBox } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ColumnBasic() {
    return (
        <Column crossAlignment={CrossAxisAlignment.Start}>
            <SizedBox height={50} />
            <SizedBox height={30} />
        </Column>
    );
}

renderRoot(ColumnBasic);
