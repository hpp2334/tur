import { Text } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function TextFontSize() {
    return (
        <>
            <Text content="Hello" fontSize={14} />
            <Text content="Hello" fontSize={28} />
        </>
    );
}

renderRoot(TextFontSize);
