import { renderRoot } from "@tur/react-renderer";
import { TextContainer, TextSpan } from "@tur/react";

function RichTextMultiSpan() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="Hello " />
      <TextSpan content="Bold" bold />
      <TextSpan content=" World" />
    </TextContainer>
  );
}

renderRoot(RichTextMultiSpan);
