import { renderRoot } from "@tur/solidjs-renderer";
import { Text } from "@tur/solidjs";

function TextWrapping() {
  return (
    <Text
      content="Hello World this is a long text that should wrap"
      fontSize={14}
    />
  );
}

renderRoot(TextWrapping);
