import { renderRoot } from "@tur/react-renderer";
import { Paragraph } from "@tur/react";

function RichTextEmpty() {
  return (
    <Paragraph fontSize={14} spans={[{ content: "" }]} />
  );
}

renderRoot(RichTextEmpty);
