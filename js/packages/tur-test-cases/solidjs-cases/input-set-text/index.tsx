import { renderRoot } from "@tur/solidjs-renderer";
import { Input, InputController } from "@tur/solidjs";

declare global {
  var __tur: {
    __ctx: unknown;
    getFirstChild(ctx: unknown, handle: object): object | null;
    setInputText(ctx: unknown, handle: object, text: string): void;
    [key: string]: unknown;
  };
}

function InputSetText() {
  return <Input controller={new InputController()} width={200} height={30} />;
}

const root = renderRoot(InputSetText);

const ctx = __tur.__ctx;
const container = __tur.getFirstChild(ctx, root);
const input = __tur.getFirstChild(ctx, container!);
if (input) __tur.setInputText(ctx, input, "hello");
