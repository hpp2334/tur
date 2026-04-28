import { renderRoot } from "@tur/solidjs-renderer";

function RichTextColor() {
  return (
    <tur_text_container fontSize={14}>
      <tur_text_span content="White" />
      <tur_text_span content="Red" color="#ff0000" />
      <tur_text_span content="Green" color="#00ff00" />
    </tur_text_container>
  );
}

renderRoot(RichTextColor);
