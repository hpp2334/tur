import { renderRoot } from "@tur/solidjs-renderer";
import { TextContainer, TextSpan } from "@tur/solidjs";

function RichTextSingleSpan() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="Hello World" />
    </TextContainer>
  );
}

renderRoot(RichTextSingleSpan);
