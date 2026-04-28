import { renderRoot } from "@tur/solidjs-renderer";

function RichTextFontSize() {
  return (
    <tur_text_container fontSize={14}>
      <tur_text_span content="Small" />
      <tur_text_span content="Big" fontSize={28} />
    </tur_text_container>
  );
}

renderRoot(RichTextFontSize);
