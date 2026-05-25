import { Column, CrossAxisAlignment, PointerInteract, Text } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";
import { useState } from "react";

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
