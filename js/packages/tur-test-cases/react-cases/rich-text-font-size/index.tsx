import { renderRoot } from "@tur/react-renderer";
import { TextContainer, TextSpan } from "@tur/react";

function RichTextFontSize() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="Small" />
      <TextSpan content="Big" fontSize={28} />
    </TextContainer>
  );
}

renderRoot(RichTextFontSize);
