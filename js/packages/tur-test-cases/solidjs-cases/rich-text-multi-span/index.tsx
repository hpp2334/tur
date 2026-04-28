import { renderRoot } from "@tur/solidjs-renderer";
import { TextContainer, TextSpan } from "@tur/solidjs";

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
