import { createTextEditingController, Input } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

const controller = createTextEditingController();

function InputPlaceholder() {
    return (
        <Input
            controller={controller}
            placeholder="Type here..."
            width={200}
            height={30}
        />
    );
}

renderRoot(InputPlaceholder);
