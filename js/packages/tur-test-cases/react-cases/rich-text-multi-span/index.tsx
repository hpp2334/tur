import { renderRoot } from "@tur/react-renderer";
import { TextContainer } from "@tur/react";

function RichTextMultiSpan() {
  return (
    <TextContainer fontSize={14} spans={[{ content: "Hello " }, { content: "Bold", bold: true }, { content: " World" }]} />
  );
}

renderRoot(RichTextMultiSpan);
