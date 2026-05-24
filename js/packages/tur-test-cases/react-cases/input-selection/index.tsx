import { renderRoot } from "@tur/react-renderer";
import { Input, createTextEditingController } from "@tur/react";

const controller = createTextEditingController();

function InputSelection() {
  return <Input controller={controller} width={200} height={30} />;
}

renderRoot(InputSelection);
