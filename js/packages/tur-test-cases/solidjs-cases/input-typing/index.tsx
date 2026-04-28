import { renderRoot } from "@tur/solidjs-renderer";
import { Input, TextController } from "@tur/solidjs";

const controller = new TextController();

function InputTyping() {
  return <Input controller={controller} fontSize={14} width={200} height={30} />;
}

renderRoot(InputTyping);
