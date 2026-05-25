import { Paragraph } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function RichTextItalic() {
    return (
        <Paragraph
            fontSize={14}
            spans={[{ content: "Normal" }, { content: "Italic", italic: true }]}
        />
    );
}

renderRoot(RichTextItalic);
