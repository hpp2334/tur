import { renderRoot } from "@tur/solidjs-renderer";
import { Input, InputController } from "@tur/solidjs";

const controller = new InputController();

function InputBasic() {
  return <Input controller={controller} width={200} height={30} />;
}

renderRoot(InputBasic);
