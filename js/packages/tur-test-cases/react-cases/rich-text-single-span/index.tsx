import { renderRoot } from "@tur/react-renderer";
import { TextContainer } from "@tur/react";

function RichTextSingleSpan() {
  return (
    <TextContainer fontSize={14} spans={[{ content: "Hello World" }]} />
  );
}

renderRoot(RichTextSingleSpan);
