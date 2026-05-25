import { Text } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function TextBasic() {
    return <Text content="Hello" fontSize={14} />;
}

renderRoot(TextBasic);
