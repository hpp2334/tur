import { renderRoot } from "@tur/solidjs-renderer";
import { Input, TextController } from "@tur/solidjs";

declare global {
  var __inputCallbackLog: string[];
}

globalThis.__inputCallbackLog = [];

const controller = new TextController({
  onInput: (text) => {
    globalThis.__inputCallbackLog.push("input:" + text);
  },
  onEnter: () => {
    globalThis.__inputCallbackLog.push("enter");
  },
});

function InputCallback() {
  return <Input controller={controller} fontSize={14} width={200} height={30} />;
}

renderRoot(InputCallback);
