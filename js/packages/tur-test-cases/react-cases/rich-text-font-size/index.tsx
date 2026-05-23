import { renderRoot } from "@tur/react-renderer";
import { TextContainer } from "@tur/react";

function RichTextFontSize() {
  return (
    <TextContainer fontSize={14} spans={[{ content: "Small" }, { content: "Big", fontSize: 28 }]} />
  );
}

renderRoot(RichTextFontSize);
