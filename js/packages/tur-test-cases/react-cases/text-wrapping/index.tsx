import { renderRoot } from "@tur/react-renderer";
import { Text } from "@tur/react";

function TextWrapping() {
  return (
    <Text
      content="Hello World this is a long text that should wrap"
      fontSize={14}
    />
  );
}

renderRoot(TextWrapping);
