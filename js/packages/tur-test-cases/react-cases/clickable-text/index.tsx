import { useState } from "react";
import { renderRoot } from "@tur/react-renderer";
import { Column, Text, PointerInteract, CrossAxisAlignment } from "@tur/react";

function ClickableText() {
  const [content, setContent] = useState("before");

  return (
    <Column crossAlignment={CrossAxisAlignment.Start}>
      <PointerInteract
        onClick={() => setContent("after")}
        child={<Text content={content} queryKey={["click-text"]} />}
      />
    </Column>
  );
}

renderRoot(ClickableText);
