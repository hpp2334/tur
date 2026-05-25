import { Paragraph } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function RichTextMultiSpan() {
    return (
        <Paragraph
            fontSize={14}
            spans={[
                { content: "Hello " },
                { content: "Bold", bold: true },
                { content: " World" },
            ]}
        />
    );
}

renderRoot(RichTextMultiSpan);
