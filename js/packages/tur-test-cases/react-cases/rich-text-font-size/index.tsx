import { Paragraph } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function RichTextFontSize() {
    return (
        <Paragraph
            fontSize={14}
            spans={[{ content: "Small" }, { content: "Big", fontSize: 28 }]}
        />
    );
}

renderRoot(RichTextFontSize);
