import { renderRoot } from "@tur/react-renderer";
import { Paragraph } from "@tur/react";

function RichTextMultiSpan() {
  return (
    <Paragraph fontSize={14} spans={[{ content: "Hello " }, { content: "Bold", bold: true }, { content: " World" }]} />
  );
}

renderRoot(RichTextMultiSpan);
