import { Text } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function TextEmptyContent() {
    return <Text content="" />;
}

renderRoot(TextEmptyContent);
