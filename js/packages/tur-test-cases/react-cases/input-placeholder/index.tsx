import { renderRoot } from "@tur/react-renderer";
import { Input, createTextEditingController } from "@tur/react";

const controller = createTextEditingController();

function InputPlaceholder() {
  return <Input controller={controller} placeholder="Type here..." width={200} height={30} />;
}

renderRoot(InputPlaceholder);
