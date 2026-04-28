import { renderRoot } from "@tur/solidjs-renderer";
import { TextContainer, TextSpan } from "@tur/solidjs";

function RichTextInheritance() {
  return (
    <TextContainer fontSize={20} color="#ff0000">
      <TextSpan content="Inherited" />
      <TextSpan content="Override" fontSize={10} color="#00ff00" />
    </TextContainer>
  );
}

renderRoot(RichTextInheritance);
