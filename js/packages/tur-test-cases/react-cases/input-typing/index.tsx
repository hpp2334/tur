import { renderRoot } from "@tur/react-renderer";
import { Input, createTextEditingController } from "@tur/react";

const controller = createTextEditingController();

function InputTyping() {
  return <Input controller={controller} fontSize={14} width={200} height={30} />;
}

renderRoot(InputTyping);
