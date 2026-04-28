import { renderRoot } from "@tur/solidjs-renderer";

function RichTextSingleSpan() {
  return (
    <tur_text_container fontSize={14}>
      <tur_text_span content="Hello World" />
    </tur_text_container>
  );
}

renderRoot(RichTextSingleSpan);
