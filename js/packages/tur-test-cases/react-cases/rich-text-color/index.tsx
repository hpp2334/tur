import { renderRoot, Color } from "@tur/react-renderer";
import { TextContainer } from "@tur/react";

function RichTextColor() {
  return (
    <TextContainer fontSize={14} spans={[{ content: "White" }, { content: "Red", color: Color.hex("#ff0000") }, { content: "Green", color: Color.hex("#00ff00") }]} />
  );
}

renderRoot(RichTextColor);
