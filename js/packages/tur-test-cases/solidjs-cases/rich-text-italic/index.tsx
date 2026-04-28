import { renderRoot } from "@tur/solidjs-renderer";
import { TextContainer, TextSpan } from "@tur/solidjs";

function RichTextItalic() {
  return (
    <TextContainer fontSize={14}>
      <TextSpan content="Normal" />
      <TextSpan content="Italic" italic />
    </TextContainer>
  );
}

renderRoot(RichTextItalic);
