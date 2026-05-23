import { renderRoot } from "@tur/react-renderer";
import { Paragraph } from "@tur/react";

function RichTextItalic() {
  return (
    <Paragraph fontSize={14} spans={[{ content: "Normal" }, { content: "Italic", italic: true }]} />
  );
}

renderRoot(RichTextItalic);
