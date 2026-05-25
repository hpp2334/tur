import { createTextEditingController, Input } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

const controller = createTextEditingController();

function InputBasic() {
    return <Input controller={controller} width={200} height={30} />;
}

renderRoot(InputBasic);
