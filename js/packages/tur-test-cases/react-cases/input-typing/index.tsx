import { renderRoot } from "@tur/react-renderer";
import { Input, InputController } from "@tur/react";

const controller = new InputController();

function InputTyping() {
  return <Input controller={controller} fontSize={14} width={200} height={30} />;
}

renderRoot(InputTyping);
