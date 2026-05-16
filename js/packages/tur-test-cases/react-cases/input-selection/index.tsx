import { renderRoot } from "@tur/react-renderer";
import { Input, InputController } from "@tur/react";

const controller = new InputController();

function InputSelection() {
  return <Input controller={controller} width={200} height={30} />;
}

renderRoot(InputSelection);
