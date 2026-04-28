import { renderRoot } from "@tur/solidjs-renderer";
import { Input, TextController } from "@tur/solidjs";

const controller = new TextController();

function InputPlaceholder() {
  return <Input controller={controller} placeholder="Type here..." width={200} height={30} />;
}

renderRoot(InputPlaceholder);
