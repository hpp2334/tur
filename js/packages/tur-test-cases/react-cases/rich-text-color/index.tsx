import { renderRoot, Color } from "@tur/react-renderer";
import { TextContainer, TextSpan } from "@tur/react";

function RichTextColor() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="White" />
      <TextSpan content="Red" color={Color.hex("#ff0000")} />
      <TextSpan content="Green" color={Color.hex("#00ff00")} />
    </TextContainer>
  );
}

renderRoot(RichTextColor);
