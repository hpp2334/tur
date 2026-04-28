import { renderRoot } from "@tur/solidjs-renderer";

function RichTextInheritance() {
  return (
    <tur_text_container fontSize={20} color="#ff0000">
      <tur_text_span content="Inherited" />
      <tur_text_span content="Override" fontSize={10} color="#00ff00" />
    </tur_text_container>
  );
}

renderRoot(RichTextInheritance);
