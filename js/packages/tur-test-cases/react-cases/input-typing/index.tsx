import { createTextEditingController, Input } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

const controller = createTextEditingController();

function InputTyping() {
    return (
        <Input controller={controller} fontSize={14} width={200} height={30} />
    );
}

renderRoot(InputTyping);
