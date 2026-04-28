import { renderRoot } from "@tur/solidjs-renderer";

function RichTextBold() {
  return (
    <tur_text_container fontSize={14}>
      <tur_text_span content="Normal" />
      <tur_text_span content="Bold" bold />
    </tur_text_container>
  );
}

renderRoot(RichTextBold);
