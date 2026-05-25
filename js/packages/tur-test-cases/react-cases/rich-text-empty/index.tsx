import { Paragraph } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function RichTextEmpty() {
    return <Paragraph fontSize={14} spans={[{ content: "" }]} />;
}

renderRoot(RichTextEmpty);
