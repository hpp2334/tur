import { renderRoot } from "@tur/react-renderer";
import { Input, InputController } from "@tur/react";

declare global {
  var __inputCallbackLog: string[];
}

globalThis.__inputCallbackLog = [];

const controller = new InputController({
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
