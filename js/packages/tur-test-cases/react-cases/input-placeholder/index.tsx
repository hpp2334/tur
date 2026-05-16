import { renderRoot } from "@tur/react-renderer";
import { Input, InputController } from "@tur/react";

const controller = new InputController();

function InputPlaceholder() {
  return <Input controller={controller} placeholder="Type here..." width={200} height={30} />;
}

renderRoot(InputPlaceholder);
