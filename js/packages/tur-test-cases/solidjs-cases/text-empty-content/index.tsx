import { renderRoot } from "@tur/solidjs-renderer";
import { Text } from "@tur/solidjs";

function TextEmptyContent() {
  return <Text content="" />;
}

renderRoot(TextEmptyContent);
