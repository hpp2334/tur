import { Column, CrossAxisAlignment, Expanded, SizedBox } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function ExpandedBasic() {
    return (
        <Column crossAlignment={CrossAxisAlignment.Start}>
            <SizedBox height={50} />
            <Expanded>
                <SizedBox />
            </Expanded>
        </Column>
    );
}

renderRoot(ExpandedBasic);
