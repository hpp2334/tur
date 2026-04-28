/// <reference types="@tur/solidjs-renderer" />
import { renderRoot } from "@tur/solidjs-renderer";
import { Input, TextController } from "@tur/solidjs";

function InputSetText() {
  return <Input controller={new TextController()} width={200} height={30} />;
}

const root = renderRoot(InputSetText);

const ctx = __tur.__ctx;
const container = __tur.getFirstChild(ctx, root);
const input = __tur.getFirstChild(ctx, container!);
if (input) __tur.setInputText(ctx, input, "hello");
