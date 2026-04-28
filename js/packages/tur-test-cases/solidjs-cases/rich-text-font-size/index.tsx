import { renderRoot } from "@tur/solidjs-renderer";
import { TextContainer, TextSpan } from "@tur/solidjs";

function RichTextFontSize() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="Small" />
      <TextSpan content="Big" fontSize={28} />
    </TextContainer>
  );
}

renderRoot(RichTextFontSize);
