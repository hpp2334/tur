import { renderRoot, Color } from "@tur/react-renderer";
import { TextContainer, TextSpan } from "@tur/react";

function RichTextInheritance() {
  return (
    <TextContainer fontSize={20}>
      <TextSpan content="Inherited" color={Color.hex("#ff0000")} />
      <TextSpan content="Override" fontSize={10} color={Color.hex("#00ff00")} />
    </TextContainer>
  );
}

renderRoot(RichTextInheritance);
