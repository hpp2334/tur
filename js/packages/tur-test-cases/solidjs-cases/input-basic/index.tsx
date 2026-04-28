import { renderRoot } from "@tur/solidjs-renderer";
import { Input, TextController } from "@tur/solidjs";

const controller = new TextController();

function InputBasic() {
  return <Input controller={controller} width={200} height={30} />;
}

renderRoot(InputBasic);
