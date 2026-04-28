import { renderRoot } from "@tur/solidjs-renderer";
import { TextContainer, TextSpan } from "@tur/solidjs";

function RichTextBold() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="Normal" />
      <TextSpan content="Bold" bold />
    </TextContainer>
  );
}

renderRoot(RichTextBold);
