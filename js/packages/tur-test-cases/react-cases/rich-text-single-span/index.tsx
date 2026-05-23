import { renderRoot } from "@tur/react-renderer";
import { Paragraph } from "@tur/react";

function RichTextSingleSpan() {
  return (
    <Paragraph fontSize={14} spans={[{ content: "Hello World" }]} />
  );
}

renderRoot(RichTextSingleSpan);
