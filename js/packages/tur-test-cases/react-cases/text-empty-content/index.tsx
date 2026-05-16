import { renderRoot } from "@tur/react-renderer";
import { Text } from "@tur/react";

function TextEmptyContent() {
  return <Text content="" />;
}

renderRoot(TextEmptyContent);
