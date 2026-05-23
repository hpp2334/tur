import { renderRoot } from "@tur/react-renderer";
import { TextContainer } from "@tur/react";

function RichTextItalic() {
  return (
    <TextContainer fontSize={14} spans={[{ content: "Normal" }, { content: "Italic", italic: true }]} />
  );
}

renderRoot(RichTextItalic);
