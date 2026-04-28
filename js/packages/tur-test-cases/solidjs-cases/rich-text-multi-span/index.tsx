import { renderRoot } from "@tur/solidjs-renderer";

function RichTextMultiSpan() {
  return (
    <tur_text_container fontSize={14}>
      <tur_text_span content="Hello " />
      <tur_text_span content="Bold" bold />
      <tur_text_span content=" World" />
    </tur_text_container>
  );
}

renderRoot(RichTextMultiSpan);
