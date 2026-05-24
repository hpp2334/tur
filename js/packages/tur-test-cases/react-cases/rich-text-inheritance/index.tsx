import { renderRoot, Color } from "@tur/react-renderer";
import { Paragraph } from "@tur/react";

function RichTextInheritance() {
  return (
    <Paragraph fontSize={20} spans={[{ content: "Inherited", color: Color.hex("#ff0000") }, { content: "Override", fontSize: 10, color: Color.hex("#00ff00") }]} />
  );
}

renderRoot(RichTextInheritance);
