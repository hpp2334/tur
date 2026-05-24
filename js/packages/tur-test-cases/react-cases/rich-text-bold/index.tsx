import { renderRoot } from "@tur/react-renderer";
import { Paragraph } from "@tur/react";

function RichTextBold() {
  return (
    <Paragraph fontSize={14} spans={[{ content: "Normal" }, { content: "Bold", bold: true }]} />
  );
}

renderRoot(RichTextBold);
