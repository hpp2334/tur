import { renderRoot } from "@tur/react-renderer";
import { Paragraph } from "@tur/react";

function RichTextFontSize() {
  return (
    <Paragraph fontSize={14} spans={[{ content: "Small" }, { content: "Big", fontSize: 28 }]} />
  );
}

renderRoot(RichTextFontSize);
