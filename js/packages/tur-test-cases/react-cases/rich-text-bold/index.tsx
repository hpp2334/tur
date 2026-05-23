import { renderRoot } from "@tur/react-renderer";
import { TextContainer } from "@tur/react";

function RichTextBold() {
  return (
    <TextContainer fontSize={14} spans={[{ content: "Normal" }, { content: "Bold", bold: true }]} />
  );
}

renderRoot(RichTextBold);
