import { renderRoot } from "@tur/solidjs-renderer";
import { Input, InputController } from "@tur/solidjs";

const controller = new InputController();

function InputPlaceholder() {
  return <Input controller={controller} placeholder="Type here..." width={200} height={30} />;
}

renderRoot(InputPlaceholder);
