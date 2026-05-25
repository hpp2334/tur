import { createTextEditingController, Input } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

const controller = createTextEditingController();

function InputSelection() {
    return <Input controller={controller} width={200} height={30} />;
}

renderRoot(InputSelection);
