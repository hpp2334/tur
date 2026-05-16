import { renderRoot } from "@tur/react-renderer";
import { TextContainer, TextSpan } from "@tur/react";

function RichTextEmpty() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="" />
    </TextContainer>
  );
}

renderRoot(RichTextEmpty);
