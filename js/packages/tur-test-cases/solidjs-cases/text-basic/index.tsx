import { renderRoot } from "@tur/solidjs-renderer";
import { Text } from "@tur/solidjs";

function TextBasic() {
  return <Text content="Hello" fontSize={14} />;
}

renderRoot(TextBasic);
