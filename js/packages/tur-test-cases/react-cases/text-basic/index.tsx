import { renderRoot } from "@tur/react-renderer";
import { Text } from "@tur/react";

function TextBasic() {
  return <Text content="Hello" fontSize={14} />;
}

renderRoot(TextBasic);
