import { Paragraph } from "@tur/react";
import { Color, renderRoot } from "@tur/react-renderer";

function RichTextInheritance() {
    return (
        <Paragraph
            fontSize={20}
            spans={[
                { content: "Inherited", color: Color.hex("#ff0000") },
                {
                    content: "Override",
                    fontSize: 10,
                    color: Color.hex("#00ff00"),
                },
            ]}
        />
    );
}

renderRoot(RichTextInheritance);
