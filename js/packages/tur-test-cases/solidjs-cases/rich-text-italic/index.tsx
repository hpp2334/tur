import { renderRoot } from "@tur/solidjs-renderer";

function RichTextItalic() {
  return (
    <tur_text_container fontSize={14}>
      <tur_text_span content="Normal" />
      <tur_text_span content="Italic" italic />
    </tur_text_container>
  );
}

renderRoot(RichTextItalic);
