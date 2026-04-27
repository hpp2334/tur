import { renderRoot } from "@tur/solidjs-renderer";
import { Column, Text, CrossAxisAlignment } from "@tur/solidjs";

function TextInColumn() {
  return (
    <Column crossAlignment={CrossAxisAlignment.End}>
      <Text content="First" fontSize={14} />
      <Text content="Second" fontSize={14} />
    </Column>
  );
}

renderRoot(TextInColumn);
