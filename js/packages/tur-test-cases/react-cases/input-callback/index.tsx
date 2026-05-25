import { createTextEditingController, Input } from "@tur/react";
import { renderRoot } from "@tur/react-renderer";

declare global {
    var __inputCallbackLog: string[];
}

globalThis.__inputCallbackLog = [];

const controller = createTextEditingController({
    onInput: (text: string, enter: boolean) => {
        globalThis.__inputCallbackLog.push(`input:${text}`);
        if (enter) {
            globalThis.__inputCallbackLog.push("enter");
        }
    },
});

function InputCallback() {
    return (
        <Input controller={controller} fontSize={14} width={200} height={30} />
    );
}

renderRoot(InputCallback);
