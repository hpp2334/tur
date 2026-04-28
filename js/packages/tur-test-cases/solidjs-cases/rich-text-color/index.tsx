import { renderRoot } from "@tur/solidjs-renderer";
import { TextContainer, TextSpan } from "@tur/solidjs";

function RichTextColor() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="White" />
      <TextSpan content="Red" color="#ff0000" />
      <TextSpan content="Green" color="#00ff00" />
    </TextContainer>
  );
}

renderRoot(RichTextColor);
