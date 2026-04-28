import { renderRoot } from "@tur/solidjs-renderer";

function RichTextEmpty() {
  return (
    <tur_text_container fontSize={14}>
      <tur_text_span content="" />
    </tur_text_container>
  );
}

renderRoot(RichTextEmpty);
