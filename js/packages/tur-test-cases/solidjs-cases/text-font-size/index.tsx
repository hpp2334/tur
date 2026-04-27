import { renderRoot } from "@tur/solidjs-renderer";
import { Text } from "@tur/solidjs";

function TextFontSize() {
  return (
    <>
      <Text content="Hello" fontSize={14} />
      <Text content="Hello" fontSize={28} />
    </>
  );
}

renderRoot(TextFontSize);
