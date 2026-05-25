import { Paragraph } from "@tur/react";
import { Color, renderRoot } from "@tur/react-renderer";

function RichTextColor() {
    return (
        <Paragraph
            fontSize={14}
            spans={[
                { content: "White" },
                { content: "Red", color: Color.hex("#ff0000") },
                { content: "Green", color: Color.hex("#00ff00") },
            ]}
        />
    );
}

renderRoot(RichTextColor);
