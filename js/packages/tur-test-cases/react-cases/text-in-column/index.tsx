import { Column, CrossAxisAlignment, Text } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function TextInColumn() {
    return (
        <Column crossAlignment={CrossAxisAlignment.End}>
            <Text content="First" fontSize={14} />
            <Text content="Second" fontSize={14} />
        </Column>
    );
}

renderRoot(TextInColumn);
