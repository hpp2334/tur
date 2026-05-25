import { Paragraph } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

function RichTextSingleSpan() {
    return <Paragraph fontSize={14} spans={[{ content: "Hello World" }]} />;
}

renderRoot(RichTextSingleSpan);
