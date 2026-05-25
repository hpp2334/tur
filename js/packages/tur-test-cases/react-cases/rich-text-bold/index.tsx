import { Paragraph } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function RichTextBold() {
    return (
        <Paragraph
            fontSize={14}
            spans={[{ content: "Normal" }, { content: "Bold", bold: true }]}
        />
    );
}

renderRoot(RichTextBold);
