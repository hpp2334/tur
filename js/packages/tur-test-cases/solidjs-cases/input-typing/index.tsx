import { renderRoot } from "@tur/solidjs-renderer";
import { Input, InputController } from "@tur/solidjs";

const controller = new InputController();

function InputTyping() {
  return <Input controller={controller} fontSize={14} width={200} height={30} />;
}

renderRoot(InputTyping);
