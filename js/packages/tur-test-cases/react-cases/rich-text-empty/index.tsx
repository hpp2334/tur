import { renderRoot } from "@tur/react-renderer";
import { TextContainer } from "@tur/react";

function RichTextEmpty() {
  return (
    <TextContainer fontSize={14} spans={[{ content: "" }]} />
  );
}

renderRoot(RichTextEmpty);
