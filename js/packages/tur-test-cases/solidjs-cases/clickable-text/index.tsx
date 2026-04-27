import { renderRoot } from "@tur/solidjs-renderer";
import { createSignal } from "solid-js";
import { Column, Text, PointerInteract, CrossAxisAlignment } from "@tur/solidjs";

function ClickableText() {
  const [content, setContent] = createSignal("before");

  return (
    <Column crossAlignment={CrossAxisAlignment.Start}>
      <PointerInteract
        onClick={() => setContent("after")}
        child={<Text content={content()} queryKey={["click-text"]} />}
      />
    </Column>
  );
}

renderRoot(ClickableText);
