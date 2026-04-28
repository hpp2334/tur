import { renderRoot } from "@tur/solidjs-renderer";
import { TextContainer, TextSpan } from "@tur/solidjs";

function RichTextEmpty() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="" />
    </TextContainer>
  );
}

renderRoot(RichTextEmpty);
