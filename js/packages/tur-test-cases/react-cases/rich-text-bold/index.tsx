import { renderRoot } from "@tur/react-renderer";
import { TextContainer, TextSpan } from "@tur/react";

function RichTextBold() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="Normal" />
      <TextSpan content="Bold" bold />
    </TextContainer>
  );
}

renderRoot(RichTextBold);
