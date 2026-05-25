import { Column, CrossAxisAlignment, Expanded, SizedBox } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ExpandedMultiple() {
    return (
        <Column crossAlignment={CrossAxisAlignment.Start}>
            <Expanded>
                <SizedBox />
            </Expanded>
            <Expanded>
                <SizedBox />
            </Expanded>
        </Column>
    );
}

renderRoot(ExpandedMultiple);
